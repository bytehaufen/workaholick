use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Datelike, Local, NaiveDate, TimeDelta};

use crate::{
    config::{RoundingConfig, RoundingType},
    domain::{
        AbsenceKind, AbsencePortion, AbsenceRecord, Record, TimelineValidationError,
        WeeklySchedule, WorkContext, reporting::TimeWindow, transition_day_state,
        work_state::DayState,
    },
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum TimeAccountingError {
    #[error(transparent)]
    TimelineValidation(#[from] TimelineValidationError),
    #[error("could not round work timestamp: {0}")]
    Rounding(#[from] chrono::RoundingError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeCategory {
    Work(WorkContext),
    Absence(AbsenceKind),
}

impl From<WorkContext> for TimeCategory {
    fn from(value: WorkContext) -> Self {
        Self::Work(value)
    }
}

impl From<AbsenceKind> for TimeCategory {
    fn from(value: AbsenceKind) -> Self {
        Self::Absence(value)
    }
}

type TimeBreakdownMap = HashMap<TimeCategory, TimeBucket>;

#[derive(Debug, Clone, Default)]
pub struct TimeBreakdown(TimeBreakdownMap);

impl TimeBreakdown {
    pub(crate) fn iter(&self) -> impl Iterator<Item = (TimeCategory, &TimeBucket)> + '_ {
        self.0.iter().map(|(kind, bucket)| (*kind, bucket))
    }

    pub(crate) fn work_active_minutes(&self) -> i64 {
        self.0
            .iter()
            .filter(|(kind, _)| matches!(kind, TimeCategory::Work(..)))
            .map(|(_, bucket)| bucket.active_time().num_minutes().max(0))
            .sum()
    }

    pub(crate) fn break_minutes(&self) -> i64 {
        self.0.values().map(|bucket| bucket.break_time().num_minutes().max(0)).sum()
    }

    pub(crate) fn absence_minutes(&self) -> i64 {
        self.0
            .iter()
            .filter(|(kind, _)| matches!(kind, TimeCategory::Absence(..)))
            .map(|(_, bucket)| bucket.active_time().num_minutes().max(0))
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TimeBucket {
    active: TimeDelta,
    break_time: TimeDelta,
    dates: HashSet<NaiveDate>,
}

impl TimeBucket {
    pub(crate) fn active_time(&self) -> TimeDelta {
        self.active
    }

    pub(crate) fn break_time(&self) -> TimeDelta {
        self.break_time
    }

    pub(crate) fn dates(&self) -> &HashSet<NaiveDate> {
        &self.dates
    }

    pub(crate) fn has_time(&self) -> bool {
        self.active > TimeDelta::zero() || self.break_time > TimeDelta::zero()
    }

    fn active_mut(&mut self) -> &mut TimeDelta {
        &mut self.active
    }

    fn break_time_mut(&mut self) -> &mut TimeDelta {
        &mut self.break_time
    }
}

fn clipped_range(
    last_timestamp: Option<DateTime<Local>>,
    timestamp: DateTime<Local>,
    window: TimeWindow,
) -> Option<(DateTime<Local>, DateTime<Local>)> {
    let start = last_timestamp.unwrap_or(timestamp).max(window.start());
    let end = timestamp.min(window.end());

    (end > start).then_some((start, end))
}

fn add_dates(bucket: &mut TimeBucket, start: DateTime<Local>, end: DateTime<Local>) {
    let first_date = start.date_naive();
    let last_date = (end - TimeDelta::seconds(1)).date_naive();

    let dates_iter = std::iter::successors(Some(first_date), |date| date.succ_opt())
        .take_while(|&date| date <= last_date);

    bucket.dates.extend(dates_iter);
}

fn add_time(
    time_breakdown_map: &mut TimeBreakdownMap,
    kind: TimeCategory,
    field: impl Fn(&mut TimeBucket) -> &mut TimeDelta,
    last_timestamp: Option<DateTime<Local>>,
    timestamp: DateTime<Local>,
    window: TimeWindow,
) {
    if let Some((start, end)) = clipped_range(last_timestamp, timestamp, window) {
        let bucket = time_breakdown_map.entry(kind).or_default();
        *field(bucket) += end - start;
        add_dates(bucket, start, end);
    }
}

fn finalize_open_bucket(
    breakdown: &mut TimeBreakdownMap,
    state: DayState,
    last_timestamp: Option<DateTime<Local>>,
    window: TimeWindow,
) {
    match state {
        DayState::Idle => {}
        DayState::Active(kind) => add_time(
            breakdown,
            TimeCategory::from(kind),
            TimeBucket::active_mut,
            last_timestamp,
            window.end(),
            window,
        ),

        DayState::Breaking(kind) => add_time(
            breakdown,
            TimeCategory::from(kind),
            TimeBucket::break_time_mut,
            last_timestamp,
            window.end(),
            window,
        ),
    }
}

fn add_absence_minutes(
    breakdown: &mut TimeBreakdownMap,
    kind: AbsenceKind,
    date: NaiveDate,
    minutes: i64,
) {
    if minutes == 0 {
        return;
    }

    let bucket = breakdown.entry(TimeCategory::Absence(kind)).or_default();
    *bucket.active_mut() += TimeDelta::minutes(minutes);
    _ = bucket.dates.insert(date);
}

pub(crate) fn calculate_time_breakdown(
    records: &[Record],
    window: TimeWindow,
    weekly_schedule: &WeeklySchedule,
    rounding_config: &RoundingConfig,
) -> Result<TimeBreakdown, TimeAccountingError> {
    struct State {
        state: DayState,
        last_timestamp: Option<DateTime<Local>>,
        breakdown: TimeBreakdownMap,
    }
    if window.is_empty() {
        return Ok(TimeBreakdown::default());
    }

    let init_state =
        State { state: DayState::Idle, last_timestamp: None, breakdown: TimeBreakdownMap::new() };

    let scan_end = window
        .end()
        .checked_add_signed(TimeDelta::minutes(i64::from(rounding_config.interval_minutes)));

    let result = records
        .iter()
        .filter_map(Record::as_work)
        .take_while(|record| scan_end.is_none_or(|end| *record.timestamp() < end))
        .try_fold(init_state, |fold_state, record| -> Result<State, TimeAccountingError> {
            let State { state, last_timestamp, mut breakdown } = fold_state;
            let next_state = transition_day_state(state, record.date(), record.command())?;

            let rounding_type = match (state, next_state) {
                (DayState::Idle, DayState::Active(_)) => RoundingType::Start,
                (DayState::Active(_), DayState::Breaking(_)) => RoundingType::BreakStart,
                (DayState::Breaking(_), DayState::Active(_)) => RoundingType::BreakEnd,
                (DayState::Active(_), DayState::Idle) => RoundingType::End,

                (DayState::Active(_), DayState::Active(_)) => {
                    return Ok(State { state, last_timestamp, breakdown });
                }
                _ => unreachable!("breakdown transitions should use the shared validator"),
            };

            let rounded = rounding_config.round(rounding_type, *record.timestamp())?;
            let timestamp = last_timestamp.map_or(rounded, |previous| rounded.max(previous));

            match state {
                DayState::Idle => {}
                DayState::Active(kind) => {
                    add_time(
                        &mut breakdown,
                        TimeCategory::from(kind),
                        TimeBucket::active_mut,
                        last_timestamp,
                        timestamp,
                        window,
                    );
                }
                DayState::Breaking(kind) => {
                    add_time(
                        &mut breakdown,
                        TimeCategory::from(kind),
                        TimeBucket::break_time_mut,
                        last_timestamp,
                        timestamp,
                        window,
                    );
                }
            };

            Ok(State { state: next_state, last_timestamp: Some(timestamp), breakdown })
        })?;

    let State { state, last_timestamp, mut breakdown } = result;
    finalize_open_bucket(&mut breakdown, state, last_timestamp, window);

    add_absence_credits(&mut breakdown, records, window, weekly_schedule);

    Ok(TimeBreakdown(breakdown))
}

fn add_absence_credits(
    breakdown: &mut TimeBreakdownMap,
    records: &[Record],
    window: TimeWindow,
    weekly_schedule: &WeeklySchedule,
) {
    let dates = TimeWindow::dates_in(window).collect::<HashSet<_>>();

    // Group records by day
    let mut absence_records_by_day = records
        .iter()
        .filter_map(Record::as_absence)
        .filter(|absence_record| dates.contains(absence_record.date()))
        .fold(HashMap::<NaiveDate, Vec<&AbsenceRecord>>::new(), |mut by_date, absence_record| {
            by_date.entry(*absence_record.date()).or_default().push(absence_record);
            by_date
        });

    absence_records_by_day.iter_mut().for_each(|(date, absence_records)| {
        absence_records.sort_by_key(|absence_record| *absence_record.id());

        let scheduled_minutes = weekly_schedule.minutes_for(date.weekday());
        let half_day_count = absence_records
            .iter()
            .filter(|record| *record.portion() == AbsencePortion::HalfDay)
            .count();
        let mut is_first_half_day = true;

        absence_records.iter().for_each(|&absence_record| {
            let minutes = match *absence_record.portion() {
                AbsencePortion::FullDay => scheduled_minutes,
                AbsencePortion::HalfDay => {
                    let remainder = if half_day_count == 2 && is_first_half_day {
                        scheduled_minutes % 2
                    } else {
                        0
                    };
                    is_first_half_day = false;

                    scheduled_minutes / 2 + remainder
                }
            };

            add_absence_minutes(breakdown, *absence_record.kind(), *date, minutes);
        });
    });
}

#[cfg(test)]
mod test {
    use crate::config::RoundingVariant;
    use crate::domain::{Record, WorkCommand};
    use crate::test_support::{self, absence_record, date, empty_schedule, ts, work_record};

    use super::*;

    fn window(start_day: u32, start_hour: u32, end_day: u32, end_hour: u32) -> TimeWindow {
        TimeWindow::new(ts(start_day, start_hour, 0), ts(end_day, end_hour, 0))
    }

    fn summarize(window: TimeWindow, records: impl IntoIterator<Item = Record>) -> TimeBreakdown {
        let records = records.into_iter().collect::<Vec<_>>();

        calculate_time_breakdown(&records, window, &empty_schedule(), &RoundingConfig::default())
            .unwrap()
    }

    fn summarize_with_schedule(
        window: TimeWindow,
        records: impl IntoIterator<Item = Record>,
        schedule: &WeeklySchedule,
    ) -> TimeBreakdown {
        let records = records.into_iter().collect::<Vec<_>>();
        calculate_time_breakdown(&records, window, schedule, &RoundingConfig::default()).unwrap()
    }

    fn hours(n: i64) -> TimeDelta {
        TimeDelta::hours(n)
    }

    fn minutes(n: i64) -> TimeDelta {
        TimeDelta::minutes(n)
    }

    fn bucket(breakdown: &TimeBreakdown, kind: TimeCategory) -> TimeBucket {
        breakdown.0.get(&kind).cloned().unwrap_or_default()
    }

    fn assert_bucket(
        breakdown: &TimeBreakdown,
        kind: TimeCategory,
        active: TimeDelta,
        break_time: TimeDelta,
    ) {
        assert_eq!(test_support::active_time(breakdown, kind), active);
        assert_eq!(test_support::break_time(breakdown, kind), break_time);
    }

    fn assert_bucket_dates(breakdown: &TimeBreakdown, kind: TimeCategory, dates: &[NaiveDate]) {
        let actual = bucket(breakdown, kind).dates;
        let expected = dates.iter().copied().collect::<HashSet<_>>();

        assert_eq!(actual, expected);
    }

    #[test]
    fn breakdown_accumulates_completed_active_and_break_time() {
        let kind = TimeCategory::Work(WorkContext::Home);

        let breakdown = summarize(
            window(28, 0, 28, 18),
            [
                work_record(1, 28, 8, 0, WorkCommand::Start { context: WorkContext::Home }),
                work_record(
                    2,
                    28,
                    9,
                    0,
                    WorkCommand::Log { title: "still working".into(), description: None },
                ),
                work_record(3, 28, 12, 0, WorkCommand::BreakStart),
                work_record(4, 28, 13, 0, WorkCommand::BreakEnd { description: None }),
                work_record(
                    5,
                    28,
                    17,
                    0,
                    WorkCommand::End { title: "done".into(), description: None },
                ),
            ],
        );

        assert_bucket(&breakdown, kind, hours(8), hours(1));
        assert_bucket_dates(&breakdown, kind, &[NaiveDate::from_ymd_opt(2026, 1, 28).unwrap()]);
    }

    #[test]
    fn breakdown_clips_open_active_time_at_window_end() {
        let kind = TimeCategory::Work(WorkContext::Home);

        let breakdown = summarize(
            window(28, 0, 28, 12),
            [
                work_record(1, 28, 8, 0, WorkCommand::Start { context: WorkContext::Home }),
                work_record(
                    2,
                    28,
                    10,
                    0,
                    WorkCommand::Log { title: "still working".into(), description: None },
                ),
            ],
        );

        assert_bucket(&breakdown, kind, hours(4), TimeDelta::zero());
    }

    #[test]
    fn breakdown_clips_open_break_time_at_window_end() {
        let kind = TimeCategory::Work(WorkContext::Home);

        let breakdown = summarize(
            window(28, 0, 28, 12),
            [
                work_record(1, 28, 8, 0, WorkCommand::Start { context: WorkContext::Home }),
                work_record(2, 28, 10, 0, WorkCommand::BreakStart),
            ],
        );

        assert_bucket(&breakdown, kind, hours(2), hours(2));
    }

    #[test]
    fn breakdown_counts_active_time_from_window_start_for_carry_in_state() {
        let kind = TimeCategory::Work(WorkContext::Home);

        let breakdown = summarize(
            window(28, 0, 30, 0),
            [
                work_record(1, 27, 10, 0, WorkCommand::Start { context: WorkContext::Home }),
                work_record(
                    2,
                    29,
                    12,
                    0,
                    WorkCommand::End { title: "done".into(), description: None },
                ),
            ],
        );

        assert_bucket(&breakdown, kind, hours(36), TimeDelta::zero());
        assert_bucket_dates(
            &breakdown,
            kind,
            &[
                NaiveDate::from_ymd_opt(2026, 1, 28).unwrap(),
                NaiveDate::from_ymd_opt(2026, 1, 29).unwrap(),
            ],
        );
    }

    #[test]
    fn breakdown_counts_break_time_from_window_start_for_carry_in_state() {
        let kind = TimeCategory::Work(WorkContext::Home);

        let breakdown = summarize(
            window(28, 0, 29, 0),
            [
                work_record(1, 27, 8, 0, WorkCommand::Start { context: WorkContext::Home }),
                work_record(2, 27, 12, 0, WorkCommand::BreakStart),
                work_record(3, 28, 10, 0, WorkCommand::BreakEnd { description: None }),
                work_record(
                    4,
                    28,
                    12,
                    0,
                    WorkCommand::End { title: "done".into(), description: None },
                ),
            ],
        );

        assert_bucket(&breakdown, kind, hours(2), hours(10));
    }

    #[test]
    fn breakdown_counts_full_window_for_open_session_without_in_range_records() {
        let kind = TimeCategory::Work(WorkContext::Home);

        let breakdown = summarize(
            window(28, 0, 29, 0),
            [work_record(1, 27, 8, 0, WorkCommand::Start { context: WorkContext::Home })],
        );

        assert_bucket(&breakdown, kind, hours(24), TimeDelta::zero());
        assert_bucket_dates(&breakdown, kind, &[NaiveDate::from_ymd_opt(2026, 1, 28).unwrap()]);
    }

    #[test]
    fn breakdown_rejects_invalid_transition() {
        let err = calculate_time_breakdown(
            &[
                work_record(1, 28, 8, 0, WorkCommand::Start { context: WorkContext::Home }),
                work_record(2, 28, 9, 0, WorkCommand::BreakEnd { description: None }),
            ],
            window(28, 0, 28, 12),
            &empty_schedule(),
            &RoundingConfig::default(),
        )
        .unwrap_err();

        assert_eq!(err.to_string(), "cannot end break when not breaking for 2026-01-28");
    }

    #[test]
    fn breakdown_credits_full_day_absence_from_schedule() {
        let window = window(12, 0, 13, 0);
        let schedule = WeeklySchedule::new([8 * 60, 0, 0, 0, 0, 0, 0]);
        let records = [absence_record(1, 12, AbsenceKind::Vacation, AbsencePortion::FullDay)];

        let breakdown = summarize_with_schedule(window, records, &schedule);

        assert_bucket(&breakdown, TimeCategory::Absence(AbsenceKind::Vacation), hours(8), hours(0));
        assert_bucket_dates(&breakdown, TimeCategory::Absence(AbsenceKind::Vacation), &[date(12)]);
    }

    #[test]
    fn breakdown_credits_half_day_absence_from_schedule() {
        let window = window(12, 0, 13, 0);
        let schedule = WeeklySchedule::new([7 * 60 + 1, 0, 0, 0, 0, 0, 0]);
        let records = [absence_record(1, 12, AbsenceKind::Vacation, AbsencePortion::HalfDay)];

        let breakdown = summarize_with_schedule(window, records, &schedule);

        assert_bucket(
            &breakdown,
            TimeCategory::Absence(AbsenceKind::Vacation),
            minutes(210),
            hours(0),
        );
        assert_bucket_dates(&breakdown, TimeCategory::Absence(AbsenceKind::Vacation), &[date(12)]);
    }

    #[test]
    fn breakdown_credits_two_half_days_as_one_full_day() {
        let window = window(12, 0, 13, 0);
        let schedule = WeeklySchedule::new([7 * 60 + 1, 0, 0, 0, 0, 0, 0]);
        let records = [
            absence_record(2, 12, AbsenceKind::Vacation, AbsencePortion::HalfDay),
            absence_record(1, 12, AbsenceKind::Sick, AbsencePortion::HalfDay),
        ];

        let breakdown = summarize_with_schedule(window, records, &schedule);

        assert_bucket(&breakdown, TimeCategory::Absence(AbsenceKind::Sick), minutes(211), hours(0));
        assert_bucket(
            &breakdown,
            TimeCategory::Absence(AbsenceKind::Vacation),
            minutes(210),
            hours(0),
        );
        assert_bucket_dates(&breakdown, TimeCategory::Absence(AbsenceKind::Sick), &[date(12)]);
        assert_bucket_dates(&breakdown, TimeCategory::Absence(AbsenceKind::Vacation), &[date(12)]);
    }

    #[test]
    fn breakdown_ignores_absence_outside_window() {
        let window = window(12, 0, 13, 0);
        let schedule = WeeklySchedule::new([8 * 60, 8 * 60, 8 * 60, 8 * 60, 8 * 60, 0, 0]);
        let records = [
            absence_record(1, 12, AbsenceKind::Vacation, AbsencePortion::HalfDay),
            absence_record(2, 13, AbsenceKind::Vacation, AbsencePortion::HalfDay),
        ];

        let breakdown = summarize_with_schedule(window, records, &schedule);

        assert_bucket(&breakdown, TimeCategory::Absence(AbsenceKind::Vacation), hours(4), hours(0));
        assert_bucket_dates(&breakdown, TimeCategory::Absence(AbsenceKind::Vacation), &[date(12)]);
    }

    #[test]
    fn breakdown_credits_current_day_absence_for_partial_window() {
        let window = window(12, 0, 12, 12);
        let schedule = WeeklySchedule::new([8 * 60, 0, 0, 0, 0, 0, 0]);
        let records = [absence_record(1, 12, AbsenceKind::Vacation, AbsencePortion::FullDay)];

        let breakdown = summarize_with_schedule(window, records, &schedule);

        assert_bucket(&breakdown, TimeCategory::Absence(AbsenceKind::Vacation), hours(8), hours(0));
        assert_bucket_dates(&breakdown, TimeCategory::Absence(AbsenceKind::Vacation), &[date(12)]);
    }

    #[test]
    fn breakdown_credits_zero_for_zero_scheduled_day() {
        let window = window(17, 0, 18, 12);
        let schedule = WeeklySchedule::new([8 * 60, 8 * 60, 8 * 60, 8 * 60, 8 * 60, 0, 0]);
        let records = [absence_record(1, 12, AbsenceKind::Vacation, AbsencePortion::FullDay)];

        let breakdown = summarize_with_schedule(window, records, &schedule);

        assert_bucket(&breakdown, TimeCategory::Absence(AbsenceKind::Vacation), hours(0), hours(0));
        assert_bucket_dates(&breakdown, TimeCategory::Absence(AbsenceKind::Vacation), &[]);
    }

    #[test]
    fn breakdown_groups_absences_of_same_kind() {
        let window = window(12, 0, 14, 0);
        let schedule = WeeklySchedule::new([8 * 60, 6 * 60, 0, 0, 0, 0, 0]);
        let records = [
            absence_record(1, 12, AbsenceKind::Vacation, AbsencePortion::FullDay),
            absence_record(1, 13, AbsenceKind::Vacation, AbsencePortion::FullDay),
        ];

        let breakdown = summarize_with_schedule(window, records, &schedule);

        assert_bucket(
            &breakdown,
            TimeCategory::Absence(AbsenceKind::Vacation),
            hours(14),
            hours(0),
        );
        assert_bucket_dates(
            &breakdown,
            TimeCategory::Absence(AbsenceKind::Vacation),
            &[date(12), date(13)],
        );
    }

    #[test]
    fn breakdown_applies_rounding_to_work_and_break_boundaries() {
        let window = window(17, 0, 18, 12);
        let schedule = empty_schedule();
        let rounding_config = RoundingConfig {
            interval_minutes: 15,
            start: RoundingVariant::Ceil,
            break_start: RoundingVariant::Floor,
            break_end: RoundingVariant::Ceil,
            end: RoundingVariant::Floor,
        };
        let records = [
            work_record(1, 17, 7, 57, WorkCommand::Start { context: WorkContext::Home }),
            work_record(2, 17, 12, 7, WorkCommand::BreakStart),
            work_record(3, 17, 12, 53, WorkCommand::BreakEnd { description: None }),
            work_record(4, 17, 17, 2, WorkCommand::End { title: "".into(), description: None }),
        ];
        let breakdown =
            calculate_time_breakdown(&records, window, &schedule, &rounding_config).unwrap();

        assert_bucket(&breakdown, TimeCategory::Work(WorkContext::Home), hours(8), hours(1));
    }
}
