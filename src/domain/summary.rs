use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Local, NaiveDate, TimeDelta};

use crate::{
    DayEntryView,
    domain::{
        ActivityKind, DayEntryValidationError, Period,
        day_entries::{DayState, transition_day_state},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub enum SummaryScope {
    Period { period: Period, offset: i64 },
    Range { from: NaiveDate, to: NaiveDate },
}

#[derive(Debug, Clone, Default)]
pub struct Summary(SummaryMap);

impl Summary {
    pub(crate) fn iter(&self) -> impl Iterator<Item = (ActivityKind, &TimeBucket)> + '_ {
        self.0.iter().map(|(kind, bucket)| (*kind, bucket))
    }

    pub(crate) fn only_work(&self) -> Self {
        Self(
            self.0
                .iter()
                .filter(|(kind, _)| matches!(kind, ActivityKind::Work { .. }))
                .map(|(kind, bucket)| (*kind, bucket.clone()))
                .collect(),
        )
    }

    pub(crate) fn total_active_minutes(&self) -> i64 {
        self.0.values().map(|bucket| bucket.active_time().num_minutes().max(0)).sum()
    }

    pub(crate) fn work_active_minutes(&self) -> i64 {
        self.0
            .iter()
            .filter(|(kind, _)| matches!(kind, ActivityKind::Work { .. }))
            .map(|(_, bucket)| bucket.active_time().num_minutes().max(0))
            .sum()
    }

    #[cfg(test)]
    pub(crate) fn active_time(&self, kind: ActivityKind) -> TimeDelta {
        self.bucket(kind).active
    }

    #[cfg(test)]
    pub(crate) fn break_time(&self, kind: ActivityKind) -> TimeDelta {
        self.bucket(kind).break_time
    }

    #[cfg(test)]
    fn bucket(&self, kind: ActivityKind) -> TimeBucket {
        self.0.get(&kind).cloned().unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryDay {
    date: NaiveDate,
    active_minutes: i64,
}

impl SummaryDay {
    pub(crate) fn new(date: NaiveDate, active_minutes: i64) -> Self {
        Self { date, active_minutes }
    }

    pub(crate) fn date(&self) -> NaiveDate {
        self.date
    }

    pub(crate) fn active_minutes(&self) -> i64 {
        self.active_minutes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryWeek {
    start: NaiveDate,
    worked_minutes: i64,
    target_minutes: Option<i64>,
}

impl SummaryWeek {
    pub(crate) fn new(start: NaiveDate, worked_minutes: i64, target_minutes: Option<i64>) -> Self {
        Self { start, worked_minutes, target_minutes }
    }

    pub(crate) fn start(&self) -> NaiveDate {
        self.start
    }

    pub(crate) fn worked_minutes(&self) -> i64 {
        self.worked_minutes
    }

    pub(crate) fn target_minutes(&self) -> Option<i64> {
        self.target_minutes
    }

    pub(crate) fn balance_minutes(&self) -> Option<i64> {
        self.target_minutes.map(|target| self.worked_minutes - target)
    }
}

#[derive(Debug, Clone)]
pub struct SummaryReport {
    window: SummaryWindow,
    summary: Summary,
    weekly_target_minutes: Option<i64>,
    weekly_days: Vec<SummaryDay>,
    weekly_balances: Vec<SummaryWeek>,
}

impl SummaryReport {
    pub(crate) fn new(window: SummaryWindow, summary: Summary) -> Self {
        Self {
            window,
            summary,
            weekly_target_minutes: None,
            weekly_days: Vec::new(),
            weekly_balances: Vec::new(),
        }
    }

    pub(crate) fn with_weekly_balance(
        window: SummaryWindow,
        summary: Summary,
        weekly_target_minutes: i64,
        weekly_days: Vec<SummaryDay>,
    ) -> Self {
        Self {
            window,
            summary,
            weekly_target_minutes: Some(weekly_target_minutes),
            weekly_days,
            weekly_balances: Vec::new(),
        }
    }

    pub(crate) fn with_weekly_breakdown(
        window: SummaryWindow,
        summary: Summary,
        weekly_balances: Vec<SummaryWeek>,
    ) -> Self {
        Self {
            window,
            summary,
            weekly_target_minutes: None,
            weekly_days: Vec::new(),
            weekly_balances,
        }
    }

    pub(crate) fn window(&self) -> SummaryWindow {
        self.window
    }

    pub(crate) fn summary(&self) -> &Summary {
        &self.summary
    }

    pub(crate) fn weekly_target_minutes(&self) -> Option<i64> {
        self.weekly_target_minutes
    }

    pub(crate) fn worked_minutes(&self) -> i64 {
        self.summary.total_active_minutes()
    }

    pub(crate) fn balance_minutes(&self) -> Option<i64> {
        self.weekly_target_minutes.map(|target| self.worked_minutes() - target)
    }

    pub(crate) fn weekly_days(&self) -> &[SummaryDay] {
        &self.weekly_days
    }

    pub(crate) fn weekly_balances(&self) -> &[SummaryWeek] {
        &self.weekly_balances
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SummaryWindow {
    start: DateTime<Local>,
    end: DateTime<Local>,
}

impl SummaryWindow {
    pub(crate) fn new(start: DateTime<Local>, end: DateTime<Local>) -> Self {
        Self { start, end }
    }

    pub(crate) fn start(self) -> DateTime<Local> {
        self.start
    }

    pub(crate) fn end(self) -> DateTime<Local> {
        self.end
    }

    pub(crate) fn is_empty(self) -> bool {
        self.start >= self.end
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

type SummaryMap = HashMap<ActivityKind, TimeBucket>;

fn clipped_range(
    last_timestamp: Option<DateTime<Local>>,
    timestamp: DateTime<Local>,
    window: SummaryWindow,
) -> Option<(DateTime<Local>, DateTime<Local>)> {
    let start = last_timestamp.unwrap_or(timestamp).max(window.start);
    let end = timestamp.min(window.end);

    (end > start).then_some((start, end))
}

fn add_dates(bucket: &mut TimeBucket, start: DateTime<Local>, end: DateTime<Local>) {
    let first_date = start.date_naive();
    let last_date = (end - TimeDelta::seconds(1)).date_naive();

    bucket.dates.extend(
        std::iter::successors(Some(first_date), |date| date.succ_opt())
            .take_while(|&date| date <= last_date),
    );
}

fn add_time(
    summary: &mut SummaryMap,
    kind: ActivityKind,
    field: impl Fn(&mut TimeBucket) -> &mut TimeDelta,
    last_timestamp: Option<DateTime<Local>>,
    timestamp: DateTime<Local>,
    window: SummaryWindow,
) {
    if let Some((start, end)) = clipped_range(last_timestamp, timestamp, window) {
        let bucket = summary.entry(kind).or_default();
        *field(bucket) += end - start;
        add_dates(bucket, start, end);
    }
}

fn finalize_open_bucket(
    summary: &mut SummaryMap,
    state: DayState,
    last_timestamp: Option<DateTime<Local>>,
    window: SummaryWindow,
) {
    match state {
        DayState::Idle => {}
        DayState::Active(kind) => {
            add_time(summary, kind, TimeBucket::active_mut, last_timestamp, window.end, window)
        }
        DayState::Breaking(kind) => {
            add_time(summary, kind, TimeBucket::break_time_mut, last_timestamp, window.end, window)
        }
    }
}

pub fn summary(
    entries: &[DayEntryView],
    window: SummaryWindow,
) -> Result<Summary, DayEntryValidationError> {
    struct State {
        state: DayState,
        last_timestamp: Option<DateTime<Local>>,
        summary: SummaryMap,
    }
    if window.is_empty() {
        return Ok(Summary::default());
    }

    let init_state =
        State { state: DayState::Idle, last_timestamp: None, summary: SummaryMap::new() };

    let result = entries.iter().filter(|entry| entry.timestamp < window.end).try_fold(
        init_state,
        |fold_state, entry| -> Result<State, DayEntryValidationError> {
            let State { state, last_timestamp, mut summary } = fold_state;
            let next_state = transition_day_state(state, entry.id, &entry.command)?;

            match (state, next_state) {
                (DayState::Idle, DayState::Active(_)) => {
                    Ok(State { state: next_state, last_timestamp: Some(entry.timestamp), summary })
                }
                (DayState::Active(kind), DayState::Breaking(_)) => {
                    add_time(
                        &mut summary,
                        kind,
                        TimeBucket::active_mut,
                        last_timestamp,
                        entry.timestamp,
                        window,
                    );
                    Ok(State { state: next_state, last_timestamp: Some(entry.timestamp), summary })
                }
                (DayState::Breaking(kind), DayState::Active(_)) => {
                    add_time(
                        &mut summary,
                        kind,
                        TimeBucket::break_time_mut,
                        last_timestamp,
                        entry.timestamp,
                        window,
                    );
                    Ok(State { state: next_state, last_timestamp: Some(entry.timestamp), summary })
                }
                (DayState::Active(kind), DayState::Idle) => {
                    add_time(
                        &mut summary,
                        kind,
                        TimeBucket::active_mut,
                        last_timestamp,
                        entry.timestamp,
                        window,
                    );
                    Ok(State { state: next_state, last_timestamp: Some(entry.timestamp), summary })
                }

                (DayState::Active(_), DayState::Active(_)) => {
                    Ok(State { state, last_timestamp, summary })
                }
                _ => unreachable!("summary transitions should use the shared validator"),
            }
        },
    )?;

    let State { state, last_timestamp, mut summary } = result;
    finalize_open_bucket(&mut summary, state, last_timestamp, window);

    Ok(Summary(summary))
}

#[cfg(test)]
mod test {
    use crate::domain::Command;

    use super::*;
    use chrono::TimeZone;

    fn timestamp(day: u32, hour: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 1, day, hour, 0, 0).unwrap()
    }

    fn window(start_day: u32, start_hour: u32, end_day: u32, end_hour: u32) -> SummaryWindow {
        SummaryWindow::new(timestamp(start_day, start_hour), timestamp(end_day, end_hour))
    }

    fn entry(id: u64, day: u32, hour: u32, command: Command) -> DayEntryView {
        DayEntryView { id: id.into(), timestamp: timestamp(day, hour), command }
    }

    fn summarize(
        window: SummaryWindow,
        entries: impl IntoIterator<Item = DayEntryView>,
    ) -> Summary {
        let entries = entries.into_iter().collect::<Vec<_>>();

        summary(&entries, window).unwrap()
    }

    fn hours(n: i64) -> TimeDelta {
        TimeDelta::hours(n)
    }

    fn assert_bucket(
        summary: &Summary,
        kind: ActivityKind,
        active: TimeDelta,
        break_time: TimeDelta,
    ) {
        assert_eq!(summary.active_time(kind), active);
        assert_eq!(summary.break_time(kind), break_time);
    }

    fn assert_bucket_dates(summary: &Summary, kind: ActivityKind, dates: &[NaiveDate]) {
        let actual = summary.bucket(kind).dates;
        let expected = dates.iter().copied().collect::<HashSet<_>>();

        assert_eq!(actual, expected);
    }

    #[test]
    fn summary_accumulates_completed_active_and_break_time() {
        let kind = ActivityKind::Vacation;

        let summary = summarize(
            window(28, 0, 28, 18),
            [
                entry(1, 28, 8, Command::Start { kind }),
                entry(2, 28, 9, Command::Log { title: "still working".into(), description: None }),
                entry(3, 28, 12, Command::BreakStart),
                entry(4, 28, 13, Command::BreakEnd { description: None }),
                entry(5, 28, 17, Command::End { title: "done".into(), description: None }),
            ],
        );

        assert_bucket(&summary, kind, hours(8), hours(1));
        assert_bucket_dates(&summary, kind, &[NaiveDate::from_ymd_opt(2026, 1, 28).unwrap()]);
    }

    #[test]
    fn summary_clips_open_active_time_at_window_end() {
        let kind = ActivityKind::Vacation;

        let summary = summarize(
            window(28, 0, 28, 12),
            [
                entry(1, 28, 8, Command::Start { kind }),
                entry(2, 28, 10, Command::Log { title: "still working".into(), description: None }),
            ],
        );

        assert_bucket(&summary, kind, hours(4), TimeDelta::zero());
    }

    #[test]
    fn summary_clips_open_break_time_at_window_end() {
        let kind = ActivityKind::Vacation;

        let summary = summarize(
            window(28, 0, 28, 12),
            [entry(1, 28, 8, Command::Start { kind }), entry(2, 28, 10, Command::BreakStart)],
        );

        assert_bucket(&summary, kind, hours(2), hours(2));
    }

    #[test]
    fn summary_counts_active_time_from_window_start_for_carry_in_state() {
        let kind = ActivityKind::Vacation;

        let summary = summarize(
            window(28, 0, 30, 0),
            [
                entry(1, 27, 10, Command::Start { kind }),
                entry(2, 29, 12, Command::End { title: "done".into(), description: None }),
            ],
        );

        assert_bucket(&summary, kind, hours(36), TimeDelta::zero());
        assert_bucket_dates(
            &summary,
            kind,
            &[
                NaiveDate::from_ymd_opt(2026, 1, 28).unwrap(),
                NaiveDate::from_ymd_opt(2026, 1, 29).unwrap(),
            ],
        );
    }

    #[test]
    fn summary_counts_break_time_from_window_start_for_carry_in_state() {
        let kind = ActivityKind::Vacation;

        let summary = summarize(
            window(28, 0, 29, 0),
            [
                entry(1, 27, 8, Command::Start { kind }),
                entry(2, 27, 12, Command::BreakStart),
                entry(3, 28, 10, Command::BreakEnd { description: None }),
                entry(4, 28, 12, Command::End { title: "done".into(), description: None }),
            ],
        );

        assert_bucket(&summary, kind, hours(2), hours(10));
    }

    #[test]
    fn summary_counts_full_window_for_open_session_without_in_range_entries() {
        let kind = ActivityKind::Vacation;

        let summary = summarize(window(28, 0, 29, 0), [entry(1, 27, 8, Command::Start { kind })]);

        assert_bucket(&summary, kind, hours(24), TimeDelta::zero());
        assert_bucket_dates(&summary, kind, &[NaiveDate::from_ymd_opt(2026, 1, 28).unwrap()]);
    }

    #[test]
    fn summary_rejects_invalid_transition() {
        let err = summary(
            &[
                entry(1, 28, 8, Command::Start { kind: ActivityKind::Vacation }),
                entry(2, 28, 9, Command::BreakEnd { description: None }),
            ],
            window(28, 0, 28, 12),
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "Day entries validation failed: Entry 2 - cannot end break when not breaking"
        );
    }
}
