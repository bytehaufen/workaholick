use std::collections::HashMap;

use chrono::{DateTime, Local, NaiveDate, TimeDelta};

use crate::{
    DayEntryView,
    domain::{
        ActivityKind, Command, DayEntryValidationError, Period,
        day_entries::{DayState, transition_day_state},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub enum SummaryScope {
    Period(Period),
    Range { from: NaiveDate, to: NaiveDate },
}

#[derive(Debug, Clone, Default)]
pub struct Summary(SummaryMap);

impl Summary {
    pub(crate) fn iter(&self) -> impl Iterator<Item = (ActivityKind, &TimeBucket)> + '_ {
        self.0.iter().map(|(kind, bucket)| (*kind, bucket))
    }

    pub(crate) fn total_active_minutes(&self) -> i64 {
        self.0.values().map(|bucket| bucket.active_time().num_minutes().max(0)).sum()
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

#[derive(Debug, Clone)]
pub struct SummaryReport {
    window: SummaryWindow,
    summary: Summary,
    weekly_target_minutes: Option<i64>,
    weekly_days: Vec<SummaryDay>,
}

impl SummaryReport {
    pub(crate) fn new(window: SummaryWindow, summary: Summary) -> Self {
        Self { window, summary, weekly_target_minutes: None, weekly_days: Vec::new() }
    }

    pub(crate) fn with_weekly_balance(
        window: SummaryWindow,
        summary: Summary,
        weekly_target_minutes: i64,
        weekly_days: Vec<SummaryDay>,
    ) -> Self {
        Self { window, summary, weekly_target_minutes: Some(weekly_target_minutes), weekly_days }
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
}

impl TimeBucket {
    pub(crate) fn active_time(&self) -> TimeDelta {
        self.active
    }

    pub(crate) fn break_time(&self) -> TimeDelta {
        self.break_time
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

fn clipped_duration(
    last_timestamp: Option<DateTime<Local>>,
    timestamp: DateTime<Local>,
    window: SummaryWindow,
) -> TimeDelta {
    let start = last_timestamp.unwrap_or(timestamp).max(window.start);
    let end = timestamp.min(window.end);

    if end > start { end - start } else { TimeDelta::zero() }
}

fn add_time(
    summary: &mut SummaryMap,
    kind: ActivityKind,
    field: impl Fn(&mut TimeBucket) -> &mut TimeDelta,
    last_timestamp: Option<DateTime<Local>>,
    timestamp: DateTime<Local>,
    window: SummaryWindow,
) {
    let delta = clipped_duration(last_timestamp, timestamp, window);

    if delta > TimeDelta::zero() {
        *field(summary.entry(kind).or_default()) += delta;
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

            match (state, next_state, &entry.command) {
                (DayState::Idle, DayState::Active(_), Command::Start { .. }) => {
                    Ok(State { state: next_state, last_timestamp: Some(entry.timestamp), summary })
                }
                (DayState::Active(kind), DayState::Breaking(_), Command::BreakStart) => {
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
                (DayState::Breaking(kind), DayState::Active(_), Command::BreakEnd { .. }) => {
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
                (DayState::Active(kind), DayState::Idle, Command::End { .. }) => {
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

                (DayState::Active(_), DayState::Active(_), Command::Log { .. }) => {
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
