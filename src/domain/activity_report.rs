use chrono::{DateTime, Local, NaiveDate};

use crate::domain::{
    AbsenceKind, AbsencePortion, Record, RecordId, TimelineValidationError, WorkCommand,
    reporting::TimeWindow, transition_day_state, work_state::DayState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkItemRow {
    id: RecordId,
    date: NaiveDate,
    start: DateTime<Local>,
    end: DateTime<Local>,
    duration_minutes: i64,
    title: String,
    description: Option<String>,
}

impl WorkItemRow {
    pub(crate) fn new(
        id: RecordId,
        date: NaiveDate,
        start: DateTime<Local>,
        end: DateTime<Local>,
        duration_minutes: i64,
        title: String,
        description: Option<String>,
    ) -> Self {
        Self { id, date, start, end, duration_minutes, title, description }
    }

    pub(crate) fn id(&self) -> RecordId {
        self.id
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
pub struct ActivityWorkTitleGroup {
    title: String,
    count: usize,
    duration_minutes: i64,
}

impl ActivityWorkTitleGroup {
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
pub(crate) struct AbsenceItemRow {
    pub id: RecordId,
    pub date: NaiveDate,
    pub kind: AbsenceKind,
    pub portion: AbsencePortion,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActivityReport {
    window: TimeWindow,
    work_items: Vec<WorkItemRow>,
    absence_items: Vec<AbsenceItemRow>,
}

impl ActivityReport {
    pub(crate) fn new(
        window: TimeWindow,
        work_items: Vec<WorkItemRow>,
        absence_items: Vec<AbsenceItemRow>,
    ) -> Self {
        Self { window, work_items, absence_items }
    }

    pub(crate) fn window(&self) -> TimeWindow {
        self.window
    }

    pub(crate) fn work_items(&self) -> &[WorkItemRow] {
        &self.work_items
    }

    pub(crate) fn absence_items(&self) -> &[AbsenceItemRow] {
        &self.absence_items
    }

    pub(crate) fn group_by_title(&self) -> Vec<ActivityWorkTitleGroup> {
        let mut by_title = self.work_items.iter().fold(
            std::collections::HashMap::<String, (usize, i64)>::new(),
            |mut grouped, row| {
                let record = grouped.entry(row.title.clone()).or_insert((0, 0));
                record.0 += 1;
                record.1 += row.duration_minutes;
                grouped
            },
        );

        let mut rows = by_title
            .drain()
            .map(|(title, (count, duration_minutes))| {
                ActivityWorkTitleGroup::new(title, count, duration_minutes)
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
        window: TimeWindow,
    ) {
        if let Some((start, end)) = clipped_range(segment_start, segment_end, window) {
            let _ = self.clipped_start.get_or_insert(start);
            self.duration_minutes += (end - start).num_minutes().max(0);
        }
    }

    fn finish(
        &mut self,
        id: RecordId,
        item_end: DateTime<Local>,
        title: String,
        description: Option<String>,
    ) -> Option<WorkItemRow> {
        let row = self.clipped_start.map(|start| {
            WorkItemRow::new(
                id,
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
    window: TimeWindow,
) -> Option<(DateTime<Local>, DateTime<Local>)> {
    let start = segment_start.unwrap_or(segment_end).max(window.start());
    let end = segment_end.min(window.end());

    (end > start).then_some((start, end))
}

pub fn calculate_activity_report(
    records: &[Record],
    window: TimeWindow,
) -> Result<ActivityReport, TimelineValidationError> {
    let init_state = State {
        state: DayState::Idle,
        active_segment_start: None,
        open_item: OpenItem::default(),
        rows: Vec::new(),
    };

    let work_rows = records
        .iter()
        .filter_map(Record::as_work)
        .filter(|record| *record.timestamp() < window.end())
        .try_fold(init_state, |mut state, record| -> Result<State, TimelineValidationError> {
            let next_state = transition_day_state(state.state, record.date(), record.command())?;

            match (&state.state, record.command(), next_state) {
                (DayState::Idle, WorkCommand::Start { .. }, DayState::Active(_)) => {
                    state.active_segment_start = Some(*record.timestamp());
                }
                (DayState::Active(_), WorkCommand::BreakStart, _) => {
                    state.open_item.add_segment(
                        state.active_segment_start,
                        *record.timestamp(),
                        window,
                    );
                    state.active_segment_start = None;
                }
                (DayState::Breaking(_), WorkCommand::BreakEnd { .. }, _) => {
                    state.active_segment_start = Some(*record.timestamp());
                }
                (DayState::Active(_), WorkCommand::Log { title, description }, _) => {
                    state.open_item.add_segment(
                        state.active_segment_start,
                        *record.timestamp(),
                        window,
                    );
                    if let Some(row) = state.open_item.finish(
                        *record.id(),
                        *record.timestamp(),
                        title.clone(),
                        description.clone(),
                    ) {
                        state.rows.push(row);
                    }
                    state.active_segment_start = Some(*record.timestamp());
                }
                (DayState::Active(_), WorkCommand::End { title, description }, _) => {
                    state.open_item.add_segment(
                        state.active_segment_start,
                        *record.timestamp(),
                        window,
                    );
                    if let Some(row) = state.open_item.finish(
                        *record.id(),
                        *record.timestamp(),
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
        })?
        .rows;

    let dates = TimeWindow::dates_in(window).collect::<Vec<_>>();
    let absence_rows = records
        .iter()
        .filter_map(Record::as_absence)
        .filter(|record| dates.contains(record.date()))
        .map(|absence_record| AbsenceItemRow {
            id: *absence_record.id(),
            date: *absence_record.date(),
            kind: *absence_record.kind(),
            portion: *absence_record.portion(),
            note: absence_record.note().clone(),
        })
        .collect();

    Ok(ActivityReport::new(window, work_rows, absence_rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AbsenceKind, AbsencePortion, WorkCommand, WorkContext};
    use crate::test_support::*;

    fn window(start_day: u32, start_hour: u32, end_day: u32, end_hour: u32) -> TimeWindow {
        TimeWindow::new(ts(start_day, start_hour, 0), ts(end_day, end_hour, 0))
    }

    #[test]
    fn activity_report_splits_log_and_end_into_separate_rows() {
        let activity_report = calculate_activity_report(
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
            activity_report.work_items(),
            &[
                WorkItemRow::new(
                    RecordId::from(2),
                    ts(10, 10, 0).date_naive(),
                    ts(10, 8, 0),
                    ts(10, 10, 0),
                    120,
                    "handoff".into(),
                    None,
                ),
                WorkItemRow::new(
                    RecordId::from(3),
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
    fn activity_report_excludes_breaks_from_row_duration() {
        let activity_report = calculate_activity_report(
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
            activity_report.work_items(),
            &[WorkItemRow::new(
                RecordId::from(4),
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
    fn activity_report_includes_absence_records() {
        let activity_report = calculate_activity_report(
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

        assert_eq!(activity_report.work_items().len(), 1);
        assert_eq!(activity_report.work_items()[0].title(), "done");
        assert_eq!(
            activity_report.absence_items()[0],
            AbsenceItemRow {
                id: RecordId::from(1),
                date: date(10),
                kind: AbsenceKind::Holiday,
                portion: AbsencePortion::HalfDay,
                note: None
            }
        );
    }

    #[test]
    fn activity_report_clips_rows_to_the_requested_window() {
        let activity_report = calculate_activity_report(
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
            activity_report.work_items(),
            &[WorkItemRow::new(
                RecordId::from(2),
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
    fn activity_report_ignores_trailing_open_work_without_log_or_end() {
        let activity_report = calculate_activity_report(
            &[work_record(1, 10, 8, 0, WorkCommand::Start { context: WorkContext::Travel })],
            window(10, 0, 11, 0),
        )
        .unwrap();

        assert!(activity_report.work_items().is_empty());
    }

    #[test]
    fn group_by_title_aggregates_exact_matches_only() {
        let activity_report = ActivityReport::new(
            window(10, 0, 11, 0),
            vec![
                WorkItemRow::new(
                    RecordId::from(0),
                    ts(10, 10, 0).date_naive(),
                    ts(10, 8, 0),
                    ts(10, 10, 0),
                    120,
                    "review".into(),
                    None,
                ),
                WorkItemRow::new(
                    RecordId::from(1),
                    ts(10, 12, 0).date_naive(),
                    ts(10, 10, 0),
                    ts(10, 12, 0),
                    120,
                    "review".into(),
                    Some("PR".into()),
                ),
                WorkItemRow::new(
                    RecordId::from(2),
                    ts(10, 13, 0).date_naive(),
                    ts(10, 12, 0),
                    ts(10, 13, 0),
                    60,
                    "Review".into(),
                    None,
                ),
            ],
            vec![],
        );

        assert_eq!(
            activity_report.group_by_title(),
            vec![
                ActivityWorkTitleGroup::new("review".into(), 2, 240),
                ActivityWorkTitleGroup::new("Review".into(), 1, 60),
            ]
        );
    }
}
