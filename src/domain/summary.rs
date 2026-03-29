use std::collections::HashMap;

use chrono::{DateTime, Local, NaiveDate, TimeDelta};

use crate::{
    DayEntryView,
    domain::{
        ActivityKind, Command, CommandLabel, DayEntryValidationError, DayEntryValidationIssue,
        Period, TransitionReason, day_entries::DayState,
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
    pub(crate) fn active_time(&self, kind: ActivityKind) -> TimeDelta {
        self.bucket(kind).active
    }

    pub(crate) fn break_time(&self, kind: ActivityKind) -> TimeDelta {
        self.bucket(kind).break_time
    }

    fn bucket(&self, kind: ActivityKind) -> TimeBucket {
        self.0.get(&kind).cloned().unwrap_or_default()
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
    fn active(&mut self) -> &mut TimeDelta {
        &mut self.active
    }

    fn break_time(&mut self) -> &mut TimeDelta {
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
            add_time(summary, kind, TimeBucket::active, last_timestamp, window.end, window)
        }
        DayState::Breaking(kind) => {
            add_time(summary, kind, TimeBucket::break_time, last_timestamp, window.end, window)
        }
    }
}

pub fn summary(entries: &[DayEntryView], window: SummaryWindow) -> anyhow::Result<Summary> {
    struct State {
        state: DayState,
        last_timestamp: Option<DateTime<Local>>,
        summary: SummaryMap,
    }
    use DayState::*;

    if window.is_empty() {
        return Ok(Summary::default());
    }

    let init_state =
        State { state: DayState::Idle, last_timestamp: None, summary: SummaryMap::new() };

    let result = entries.iter().filter(|entry| entry.timestamp < window.end).try_fold(
        init_state,
        |fold_state, entry| {
            let State { state, last_timestamp, mut summary } = fold_state;

            match (state, &entry.command) {
                (Idle, Command::Start { kind }) => Ok(State {
                    state: Active(*kind),
                    last_timestamp: Some(entry.timestamp),
                    summary,
                }),
                (Active(kind), Command::BreakStart) => {
                    add_time(
                        &mut summary,
                        kind,
                        TimeBucket::active,
                        last_timestamp,
                        entry.timestamp,
                        window,
                    );
                    Ok(State {
                        state: Breaking(kind),
                        last_timestamp: Some(entry.timestamp),
                        summary,
                    })
                }
                (Breaking(kind), Command::BreakEnd { .. }) => {
                    add_time(
                        &mut summary,
                        kind,
                        TimeBucket::break_time,
                        last_timestamp,
                        entry.timestamp,
                        window,
                    );
                    Ok(State {
                        state: Active(kind),
                        last_timestamp: Some(entry.timestamp),
                        summary,
                    })
                }
                (Active(kind), Command::End { .. }) => {
                    add_time(
                        &mut summary,
                        kind,
                        TimeBucket::active,
                        last_timestamp,
                        entry.timestamp,
                        window,
                    );
                    Ok(State { state: Idle, last_timestamp: Some(entry.timestamp), summary })
                }

                (Active(_), Command::Log { .. }) => Ok(State { state, last_timestamp, summary }),

                (Active(_), Command::BreakEnd { .. }) => {
                    Err(DayEntryValidationError::new(DayEntryValidationIssue::InvalidTransition {
                        entry_id: entry.id,
                        attempted: CommandLabel::BreakEnd,
                        reason: TransitionReason::NotBreaking,
                    }))
                }

                (
                    Breaking(_),
                    Command::Start { .. } | Command::End { .. } | Command::Log { .. },
                ) => {
                    Err(DayEntryValidationError::new(DayEntryValidationIssue::InvalidTransition {
                        entry_id: entry.id,
                        attempted: CommandLabel::from(&entry.command),
                        reason: TransitionReason::WhileBreaking,
                    }))
                }

                (Breaking(_), Command::BreakStart) => {
                    Err(DayEntryValidationError::new(DayEntryValidationIssue::InvalidTransition {
                        entry_id: entry.id,
                        attempted: CommandLabel::BreakStart,
                        reason: TransitionReason::AlreadyBreaking,
                    }))
                }

                (Active(activity_kind), Command::Start { kind }) => {
                    Err(DayEntryValidationError::new(DayEntryValidationIssue::InvalidTransition {
                        entry_id: entry.id,
                        attempted: CommandLabel::StartWithKind(*kind),
                        reason: TransitionReason::AlreadyActive(activity_kind),
                    }))
                }

                (Idle, command) => {
                    Err(DayEntryValidationError::new(DayEntryValidationIssue::InvalidTransition {
                        entry_id: entry.id,
                        attempted: command.into(),
                        reason: TransitionReason::WhileIdle,
                    }))
                }
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
