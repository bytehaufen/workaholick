use chrono::{DateTime, Local, NaiveDate};

use crate::{
    DayEntryView,
    domain::{
        Command, DayEntryValidationError,
        day_entries::{DayState, transition_day_state},
        summary::SummaryWindow,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportGroupBy {
    Title,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkItemRow {
    date: NaiveDate,
    start: DateTime<Local>,
    end: DateTime<Local>,
    duration_minutes: i64,
    title: String,
    description: Option<String>,
}

impl WorkItemRow {
    pub(crate) fn new(
        date: NaiveDate,
        start: DateTime<Local>,
        end: DateTime<Local>,
        duration_minutes: i64,
        title: String,
        description: Option<String>,
    ) -> Self {
        Self { date, start, end, duration_minutes, title, description }
    }

    pub(crate) fn date(&self) -> NaiveDate {
        self.date
    }

    pub(crate) fn start(&self) -> DateTime<Local> {
        self.start
    }

    pub(crate) fn end(&self) -> DateTime<Local> {
        self.end
    }

    pub(crate) fn duration_minutes(&self) -> i64 {
        self.duration_minutes
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportTitleGroup {
    title: String,
    count: usize,
    duration_minutes: i64,
}

impl ReportTitleGroup {
    fn new(title: String, count: usize, duration_minutes: i64) -> Self {
        Self { title, count, duration_minutes }
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn count(&self) -> usize {
        self.count
    }

    pub(crate) fn duration_minutes(&self) -> i64 {
        self.duration_minutes
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkItemReport {
    window: SummaryWindow,
    rows: Vec<WorkItemRow>,
}

impl WorkItemReport {
    pub(crate) fn new(window: SummaryWindow, rows: Vec<WorkItemRow>) -> Self {
        Self { window, rows }
    }

    pub(crate) fn window(&self) -> SummaryWindow {
        self.window
    }

    pub(crate) fn rows(&self) -> &[WorkItemRow] {
        &self.rows
    }

    pub(crate) fn group_by_title(&self) -> Vec<ReportTitleGroup> {
        let mut by_title = self.rows.iter().fold(
            std::collections::HashMap::<String, (usize, i64)>::new(),
            |mut grouped, row| {
                let entry = grouped.entry(row.title.clone()).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += row.duration_minutes;
                grouped
            },
        );

        let mut rows = by_title
            .drain()
            .map(|(title, (count, duration_minutes))| {
                ReportTitleGroup::new(title, count, duration_minutes)
            })
            .collect::<Vec<_>>();

        rows.sort_by(|left, right| {
            right
                .duration_minutes
                .cmp(&left.duration_minutes)
                .then_with(|| left.title.cmp(&right.title))
        });
        rows
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct OpenItem {
    clipped_start: Option<DateTime<Local>>,
    duration_minutes: i64,
}

impl OpenItem {
    fn add_segment(
        &mut self,
        segment_start: Option<DateTime<Local>>,
        segment_end: DateTime<Local>,
        window: SummaryWindow,
    ) {
        if let Some((start, end)) = clipped_range(segment_start, segment_end, window) {
            self.clipped_start.get_or_insert(start);
            self.duration_minutes += (end - start).num_minutes().max(0);
        }
    }

    fn finish(
        &mut self,
        item_end: DateTime<Local>,
        title: String,
        description: Option<String>,
    ) -> Option<WorkItemRow> {
        let row = self.clipped_start.map(|start| {
            WorkItemRow::new(
                item_end.date_naive(),
                start,
                item_end,
                self.duration_minutes,
                title,
                description,
            )
        });

        *self = Self::default();
        row.filter(|row| row.duration_minutes > 0)
    }
}

struct State {
    state: DayState,
    active_segment_start: Option<DateTime<Local>>,
    open_item: OpenItem,
    rows: Vec<WorkItemRow>,
}

fn clipped_range(
    segment_start: Option<DateTime<Local>>,
    segment_end: DateTime<Local>,
    window: SummaryWindow,
) -> Option<(DateTime<Local>, DateTime<Local>)> {
    let start = segment_start.unwrap_or(segment_end).max(window.start());
    let end = segment_end.min(window.end());

    (end > start).then_some((start, end))
}

pub fn report(
    entries: &[DayEntryView],
    window: SummaryWindow,
) -> Result<WorkItemReport, DayEntryValidationError> {
    let init_state = State {
        state: DayState::Idle,
        active_segment_start: None,
        open_item: OpenItem::default(),
        rows: Vec::new(),
    };

    let state = entries.iter().filter(|entry| entry.timestamp < window.end()).try_fold(
        init_state,
        |mut state, entry| -> Result<State, DayEntryValidationError> {
            let next_state = transition_day_state(state.state, entry.id, &entry.command)?;

            match (&state.state, &entry.command) {
                (DayState::Idle, Command::Start { .. }) => {
                    state.active_segment_start = Some(entry.timestamp);
                }
                (DayState::Active(_), Command::BreakStart) => {
                    state.open_item.add_segment(
                        state.active_segment_start,
                        entry.timestamp,
                        window,
                    );
                    state.active_segment_start = None;
                }
                (DayState::Breaking(_), Command::BreakEnd { .. }) => {
                    state.active_segment_start = Some(entry.timestamp);
                }
                (DayState::Active(_), Command::Log { title, description }) => {
                    state.open_item.add_segment(
                        state.active_segment_start,
                        entry.timestamp,
                        window,
                    );
                    if let Some(row) =
                        state.open_item.finish(entry.timestamp, title.clone(), description.clone())
                    {
                        state.rows.push(row);
                    }
                    state.active_segment_start = Some(entry.timestamp);
                }
                (DayState::Active(_), Command::End { title, description }) => {
                    state.open_item.add_segment(
                        state.active_segment_start,
                        entry.timestamp,
                        window,
                    );
                    if let Some(row) =
                        state.open_item.finish(entry.timestamp, title.clone(), description.clone())
                    {
                        state.rows.push(row);
                    }
                    state.active_segment_start = None;
                }
                _ => {}
            }

            state.state = next_state;
            Ok(state)
        },
    )?;

    Ok(WorkItemReport::new(window, state.rows))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::domain::{ActivityKind, Command, WorkContext};

    fn ts(day: u32, hour: u32, minute: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 1, day, hour, minute, 0).unwrap()
    }

    fn window(start_day: u32, start_hour: u32, end_day: u32, end_hour: u32) -> SummaryWindow {
        SummaryWindow::new(ts(start_day, start_hour, 0), ts(end_day, end_hour, 0))
    }

    fn entry(id: u64, day: u32, hour: u32, minute: u32, command: Command) -> DayEntryView {
        DayEntryView { id: id.into(), timestamp: ts(day, hour, minute), command }
    }

    #[test]
    fn report_splits_log_and_end_into_separate_rows() {
        let kind = ActivityKind::Work { context: WorkContext::Office };
        let report = report(
            &[
                entry(1, 10, 8, 0, Command::Start { kind }),
                entry(2, 10, 10, 0, Command::Log { title: "handoff".into(), description: None }),
                entry(3, 10, 12, 0, Command::End { title: "wrap".into(), description: None }),
            ],
            window(10, 0, 11, 0),
        )
        .unwrap();

        assert_eq!(
            report.rows(),
            &[
                WorkItemRow::new(
                    ts(10, 10, 0).date_naive(),
                    ts(10, 8, 0),
                    ts(10, 10, 0),
                    120,
                    "handoff".into(),
                    None,
                ),
                WorkItemRow::new(
                    ts(10, 12, 0).date_naive(),
                    ts(10, 10, 0),
                    ts(10, 12, 0),
                    120,
                    "wrap".into(),
                    None,
                ),
            ]
        );
    }

    #[test]
    fn report_excludes_breaks_from_row_duration() {
        let kind = ActivityKind::Holiday;
        let report = report(
            &[
                entry(1, 10, 8, 0, Command::Start { kind }),
                entry(2, 10, 10, 0, Command::BreakStart),
                entry(3, 10, 11, 0, Command::BreakEnd { description: None }),
                entry(4, 10, 13, 0, Command::End { title: "done".into(), description: None }),
            ],
            window(10, 0, 11, 0),
        )
        .unwrap();

        assert_eq!(
            report.rows(),
            &[WorkItemRow::new(
                ts(10, 13, 0).date_naive(),
                ts(10, 8, 0),
                ts(10, 13, 0),
                240,
                "done".into(),
                None,
            )]
        );
    }

    #[test]
    fn report_clips_rows_to_the_requested_window() {
        let kind = ActivityKind::Work { context: WorkContext::Home };
        let report = report(
            &[
                entry(1, 9, 20, 0, Command::Start { kind }),
                entry(2, 10, 10, 0, Command::End { title: "done".into(), description: None }),
            ],
            window(10, 0, 11, 0),
        )
        .unwrap();

        assert_eq!(
            report.rows(),
            &[WorkItemRow::new(
                ts(10, 10, 0).date_naive(),
                ts(10, 0, 0),
                ts(10, 10, 0),
                600,
                "done".into(),
                None,
            )]
        );
    }

    #[test]
    fn report_ignores_trailing_open_work_without_log_or_end() {
        let kind = ActivityKind::Work { context: WorkContext::Travel };
        let report =
            report(&[entry(1, 10, 8, 0, Command::Start { kind })], window(10, 0, 11, 0)).unwrap();

        assert!(report.rows().is_empty());
    }

    #[test]
    fn group_by_title_aggregates_exact_matches_only() {
        let report = WorkItemReport::new(
            window(10, 0, 11, 0),
            vec![
                WorkItemRow::new(
                    ts(10, 10, 0).date_naive(),
                    ts(10, 8, 0),
                    ts(10, 10, 0),
                    120,
                    "review".into(),
                    None,
                ),
                WorkItemRow::new(
                    ts(10, 12, 0).date_naive(),
                    ts(10, 10, 0),
                    ts(10, 12, 0),
                    120,
                    "review".into(),
                    Some("PR".into()),
                ),
                WorkItemRow::new(
                    ts(10, 13, 0).date_naive(),
                    ts(10, 12, 0),
                    ts(10, 13, 0),
                    60,
                    "Review".into(),
                    None,
                ),
            ],
        );

        assert_eq!(
            report.group_by_title(),
            vec![
                ReportTitleGroup::new("review".into(), 2, 240),
                ReportTitleGroup::new("Review".into(), 1, 60),
            ]
        );
    }
}
