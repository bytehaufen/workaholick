use chrono::{DateTime, Local, NaiveDate};

use crate::domain::{
    DayRecordValidationError, Record, WorkCommand, summary::SummaryWindow, transition_day_state,
    work_transition::DayState,
};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    strum::Display,
    strum::EnumString,
    strum::VariantNames,
)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "snake_case")]
pub enum ReportGroupBy {
    Title,
    TitleDescription,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportTitleDescriptionGroup {
    title: String,
    description: Option<String>,
    count: usize,
    duration_minutes: i64,
}

impl ReportTitleDescriptionGroup {
    fn new(
        title: String,
        description: Option<String>,
        count: usize,
        duration_minutes: i64,
    ) -> Self {
        Self { title, description, count, duration_minutes }
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn description(&self) -> Option<&str> {
        self.description.as_deref()
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

    pub(crate) fn group_by_title_description(&self) -> Vec<ReportTitleDescriptionGroup> {
        let mut by_title_description = self.rows.iter().fold(
            std::collections::HashMap::<(String, Option<String>), (usize, i64)>::new(),
            |mut grouped, row| {
                let entry =
                    grouped.entry((row.title.clone(), row.description.clone())).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += row.duration_minutes;
                grouped
            },
        );

        let mut rows = by_title_description
            .drain()
            .map(|((title, description), (count, duration_minutes))| {
                ReportTitleDescriptionGroup::new(title, description, count, duration_minutes)
            })
            .collect::<Vec<_>>();

        rows.sort_by(|left, right| {
            right
                .duration_minutes
                .cmp(&left.duration_minutes)
                .then_with(|| left.title.cmp(&right.title))
                .then_with(|| {
                    left.description
                        .as_deref()
                        .unwrap_or("")
                        .cmp(right.description.as_deref().unwrap_or(""))
                })
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
            let _ = self.clipped_start.get_or_insert(start);
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
    records: &[Record],
    window: SummaryWindow,
) -> Result<WorkItemReport, DayRecordValidationError> {
    let init_state = State {
        state: DayState::Idle,
        active_segment_start: None,
        open_item: OpenItem::default(),
        rows: Vec::new(),
    };

    // FIXME: Need to respect Absence
    let state = records
        .iter()
        .filter_map(Record::as_work)
        .filter(|entry| *entry.timestamp() < window.end())
        .try_fold(init_state, |mut state, entry| -> Result<State, DayRecordValidationError> {
            let next_state = transition_day_state(state.state, *entry.id(), entry.command())?;

            match (&state.state, entry.command(), next_state) {
                (DayState::Idle, WorkCommand::Start { .. }, DayState::Active(_)) => {
                    state.active_segment_start = Some(*entry.timestamp());
                }
                (DayState::Active(_), WorkCommand::BreakStart, _) => {
                    state.open_item.add_segment(
                        state.active_segment_start,
                        *entry.timestamp(),
                        window,
                    );
                    state.active_segment_start = None;
                }
                (DayState::Breaking(_), WorkCommand::BreakEnd { .. }, _) => {
                    state.active_segment_start = Some(*entry.timestamp());
                }
                (DayState::Active(_), WorkCommand::Log { title, description }, _) => {
                    state.open_item.add_segment(
                        state.active_segment_start,
                        *entry.timestamp(),
                        window,
                    );
                    if let Some(row) = state.open_item.finish(
                        *entry.timestamp(),
                        title.clone(),
                        description.clone(),
                    ) {
                        state.rows.push(row);
                    }
                    state.active_segment_start = Some(*entry.timestamp());
                }
                (DayState::Active(_), WorkCommand::End { title, description }, _) => {
                    state.open_item.add_segment(
                        state.active_segment_start,
                        *entry.timestamp(),
                        window,
                    );
                    if let Some(row) = state.open_item.finish(
                        *entry.timestamp(),
                        title.clone(),
                        description.clone(),
                    ) {
                        state.rows.push(row);
                    }
                    state.active_segment_start = None;
                }
                _ => {}
            }

            state.state = next_state;
            Ok(state)
        })?;

    Ok(WorkItemReport::new(window, state.rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AbsenceKind, AbsencePortion, WorkCommand, WorkContext};
    use crate::test_support::*;

    fn window(start_day: u32, start_hour: u32, end_day: u32, end_hour: u32) -> SummaryWindow {
        SummaryWindow::new(ts(start_day, start_hour, 0), ts(end_day, end_hour, 0))
    }

    #[test]
    fn report_splits_log_and_end_into_separate_rows() {
        let report = report(
            &[
                work_record(1, 10, 8, 0, WorkCommand::Start { context: WorkContext::Office }),
                work_record(
                    2,
                    10,
                    10,
                    0,
                    WorkCommand::Log { title: "handoff".into(), description: None },
                ),
                work_record(
                    3,
                    10,
                    12,
                    0,
                    WorkCommand::End { title: "wrap".into(), description: None },
                ),
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
        let report = report(
            &[
                work_record(1, 10, 8, 0, WorkCommand::Start { context: WorkContext::Travel }),
                work_record(2, 10, 10, 0, WorkCommand::BreakStart),
                work_record(3, 10, 11, 0, WorkCommand::BreakEnd { description: None }),
                work_record(
                    4,
                    10,
                    13,
                    0,
                    WorkCommand::End { title: "done".into(), description: None },
                ),
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
    fn report_ignores_absence_records() {
        let report = report(
            &[
                absence_record(1, 10, AbsenceKind::Holiday, AbsencePortion::HalfDay),
                work_record(2, 10, 8, 0, WorkCommand::Start { context: WorkContext::Office }),
                work_record(
                    3,
                    10,
                    12,
                    0,
                    WorkCommand::End { title: "done".into(), description: None },
                ),
            ],
            window(10, 0, 11, 0),
        )
        .unwrap();

        assert_eq!(report.rows().len(), 1);
        assert_eq!(report.rows()[0].title(), "done");
    }

    #[test]
    fn report_clips_rows_to_the_requested_window() {
        let report = report(
            &[
                work_record(1, 9, 20, 0, WorkCommand::Start { context: WorkContext::Home }),
                work_record(
                    2,
                    10,
                    10,
                    0,
                    WorkCommand::End { title: "done".into(), description: None },
                ),
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
        let report = report(
            &[work_record(1, 10, 8, 0, WorkCommand::Start { context: WorkContext::Travel })],
            window(10, 0, 11, 0),
        )
        .unwrap();

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

    #[test]
    fn group_by_title_description_splits_distinct_descriptions() {
        let report = WorkItemReport::new(
            window(10, 0, 11, 0),
            vec![
                WorkItemRow::new(
                    ts(10, 10, 0).date_naive(),
                    ts(10, 8, 0),
                    ts(10, 10, 0),
                    120,
                    "review".into(),
                    Some("PR #184".into()),
                ),
                WorkItemRow::new(
                    ts(10, 12, 0).date_naive(),
                    ts(10, 10, 0),
                    ts(10, 12, 0),
                    60,
                    "review".into(),
                    Some("PR #184".into()),
                ),
                WorkItemRow::new(
                    ts(10, 13, 0).date_naive(),
                    ts(10, 12, 0),
                    ts(10, 13, 0),
                    60,
                    "review".into(),
                    Some("architecture notes".into()),
                ),
                WorkItemRow::new(
                    ts(10, 14, 0).date_naive(),
                    ts(10, 13, 0),
                    ts(10, 14, 0),
                    30,
                    "review".into(),
                    None,
                ),
            ],
        );

        assert_eq!(
            report.group_by_title_description(),
            vec![
                ReportTitleDescriptionGroup::new("review".into(), Some("PR #184".into()), 2, 180),
                ReportTitleDescriptionGroup::new(
                    "review".into(),
                    Some("architecture notes".into()),
                    1,
                    60,
                ),
                ReportTitleDescriptionGroup::new("review".into(), None, 1, 30),
            ]
        );
    }
}
