use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Local, NaiveDate, TimeDelta};

use crate::domain::{
    AbsenceKind, Record, TimeWindow, TimelineValidationError, WorkContext, transition_day_state,
    work_state::DayState,
};

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

    pub(crate) fn total_active_minutes(&self) -> i64 {
        self.0.values().map(|bucket| bucket.active_time().num_minutes().max(0)).sum()
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

    #[cfg(test)]
    pub(crate) fn active_time(&self, kind: TimeCategory) -> TimeDelta {
        self.bucket(kind).active
    }

    #[cfg(test)]
    pub(crate) fn break_time(&self, kind: TimeCategory) -> TimeDelta {
        self.bucket(kind).break_time
    }

    #[cfg(test)]
    fn bucket(&self, kind: TimeCategory) -> TimeBucket {
        self.0.get(&kind).cloned().unwrap_or_default()
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

pub fn calculate_time_breakdown(
    records: &[Record],
    window: TimeWindow,
) -> Result<TimeBreakdown, TimelineValidationError> {
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

    let result = records
        .iter()
        .filter_map(Record::as_work)
        .filter(|record| *record.timestamp() < window.end())
        .try_fold(init_state, |fold_state, record| -> Result<State, TimelineValidationError> {
            let State { state, last_timestamp, mut breakdown } = fold_state;
            let next_state = transition_day_state(state, record.date(), record.command())?;

            match (state, next_state) {
                (DayState::Idle, DayState::Active(_)) => Ok(State {
                    state: next_state,
                    last_timestamp: Some(*record.timestamp()),
                    breakdown,
                }),
                (DayState::Active(kind), DayState::Breaking(_)) => {
                    add_time(
                        &mut breakdown,
                        TimeCategory::from(kind),
                        TimeBucket::active_mut,
                        last_timestamp,
                        *record.timestamp(),
                        window,
                    );
                    Ok(State {
                        state: next_state,
                        last_timestamp: Some(*record.timestamp()),
                        breakdown,
                    })
                }
                (DayState::Breaking(kind), DayState::Active(_)) => {
                    add_time(
                        &mut breakdown,
                        TimeCategory::from(kind),
                        TimeBucket::break_time_mut,
                        last_timestamp,
                        *record.timestamp(),
                        window,
                    );
                    Ok(State {
                        state: next_state,
                        last_timestamp: Some(*record.timestamp()),
                        breakdown,
                    })
                }
                (DayState::Active(kind), DayState::Idle) => {
                    add_time(
                        &mut breakdown,
                        TimeCategory::from(kind),
                        TimeBucket::active_mut,
                        last_timestamp,
                        *record.timestamp(),
                        window,
                    );
                    Ok(State {
                        state: next_state,
                        last_timestamp: Some(*record.timestamp()),
                        breakdown,
                    })
                }

                (DayState::Active(_), DayState::Active(_)) => {
                    Ok(State { state, last_timestamp, breakdown })
                }
                _ => unreachable!("breakdown transitions should use the shared validator"),
            }
        })?;

    let State { state, last_timestamp, mut breakdown } = result;
    finalize_open_bucket(&mut breakdown, state, last_timestamp, window);

    Ok(TimeBreakdown(breakdown))
}

#[cfg(test)]
mod test {
    use crate::domain::{Record, WorkCommand};
    use crate::test_support::ts;

    use super::*;

    fn window(start_day: u32, start_hour: u32, end_day: u32, end_hour: u32) -> TimeWindow {
        TimeWindow::new(ts(start_day, start_hour, 0), ts(end_day, end_hour, 0))
    }

    fn record(id: u64, day: u32, hour: u32, command: WorkCommand) -> Record {
        Record::from_input(
            id.into(),
            crate::domain::RecordInput::Work(crate::domain::WorkRecordInput::new(
                ts(day, hour, 0),
                command,
            )),
        )
    }

    fn summarize(window: TimeWindow, records: impl IntoIterator<Item = Record>) -> TimeBreakdown {
        let records = records.into_iter().collect::<Vec<_>>();

        calculate_time_breakdown(&records, window).unwrap()
    }

    fn hours(n: i64) -> TimeDelta {
        TimeDelta::hours(n)
    }

    fn assert_bucket(
        breakdown: &TimeBreakdown,
        kind: TimeCategory,
        active: TimeDelta,
        break_time: TimeDelta,
    ) {
        assert_eq!(breakdown.active_time(kind), active);
        assert_eq!(breakdown.break_time(kind), break_time);
    }

    fn assert_bucket_dates(breakdown: &TimeBreakdown, kind: TimeCategory, dates: &[NaiveDate]) {
        let actual = breakdown.bucket(kind).dates;
        let expected = dates.iter().copied().collect::<HashSet<_>>();

        assert_eq!(actual, expected);
    }

    #[test]
    fn breakdown_accumulates_completed_active_and_break_time() {
        let kind = TimeCategory::Work(WorkContext::Home);

        let breakdown = summarize(
            window(28, 0, 28, 18),
            [
                record(1, 28, 8, WorkCommand::Start { context: WorkContext::Home }),
                record(
                    2,
                    28,
                    9,
                    WorkCommand::Log { title: "still working".into(), description: None },
                ),
                record(3, 28, 12, WorkCommand::BreakStart),
                record(4, 28, 13, WorkCommand::BreakEnd { description: None }),
                record(5, 28, 17, WorkCommand::End { title: "done".into(), description: None }),
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
                record(1, 28, 8, WorkCommand::Start { context: WorkContext::Home }),
                record(
                    2,
                    28,
                    10,
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
                record(1, 28, 8, WorkCommand::Start { context: WorkContext::Home }),
                record(2, 28, 10, WorkCommand::BreakStart),
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
                record(1, 27, 10, WorkCommand::Start { context: WorkContext::Home }),
                record(2, 29, 12, WorkCommand::End { title: "done".into(), description: None }),
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
                record(1, 27, 8, WorkCommand::Start { context: WorkContext::Home }),
                record(2, 27, 12, WorkCommand::BreakStart),
                record(3, 28, 10, WorkCommand::BreakEnd { description: None }),
                record(4, 28, 12, WorkCommand::End { title: "done".into(), description: None }),
            ],
        );

        assert_bucket(&breakdown, kind, hours(2), hours(10));
    }

    #[test]
    fn breakdown_counts_full_window_for_open_session_without_in_range_records() {
        let kind = TimeCategory::Work(WorkContext::Home);

        let breakdown = summarize(
            window(28, 0, 29, 0),
            [record(1, 27, 8, WorkCommand::Start { context: WorkContext::Home })],
        );

        assert_bucket(&breakdown, kind, hours(24), TimeDelta::zero());
        assert_bucket_dates(&breakdown, kind, &[NaiveDate::from_ymd_opt(2026, 1, 28).unwrap()]);
    }

    #[test]
    fn breakdown_rejects_invalid_transition() {
        let err = calculate_time_breakdown(
            &[
                record(1, 28, 8, WorkCommand::Start { context: WorkContext::Home }),
                record(2, 28, 9, WorkCommand::BreakEnd { description: None }),
            ],
            window(28, 0, 28, 12),
        )
        .unwrap_err();

        assert_eq!(err.to_string(), "cannot end break when not breaking for 2026-01-28");
    }
}
