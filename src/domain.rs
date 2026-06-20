mod absence;
mod period;
mod record;
mod record_set;
pub(crate) mod report;
mod serde_datetime;
mod summary;
mod timeline_validation;
mod work;
mod work_timeline_validation;
mod work_transition;

pub(crate) use absence::{AbsenceKind, AbsencePortion, AbsenceRecord, AbsenceRecordInput};
pub(crate) use period::Period;
pub(crate) use record::{Record, RecordId, RecordInput};
pub(crate) use record_set::{ChangeOutcome, RecordChange, RecordSet};
pub(crate) use report::{ReportGroupBy, WorkItemReport, report};
pub(crate) use summary::{
    SummaryDay, SummaryKind, SummaryReport, SummaryScope, SummaryWeek, SummaryWindow, summary,
};
pub(crate) use timeline_validation::validate_records;
pub(crate) use work::{WorkCommand, WorkContext, WorkRecord, WorkRecordInput};
pub(crate) use work_transition::{
    CommandLabel, DayRecordValidationError, DayRecordValidationIssue, TransitionReason,
    transition_day_state,
};
