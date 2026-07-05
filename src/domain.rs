mod absence;
mod period;
mod record;
mod record_set;
pub(crate) mod report;
mod reporting;
mod serde_datetime;
mod timeline_validation;
mod timesheet;
mod work;
mod work_state;

pub(crate) use absence::{AbsenceKind, AbsencePortion, AbsenceRecord, AbsenceRecordInput};
pub(crate) use period::Period;
pub(crate) use record::{Record, RecordId, RecordInput};
pub(crate) use record_set::{ChangeOutcome, RecordChange, RecordSet};
pub(crate) use report::{ActivityGroupBy, WorkItemReport, report};
pub(crate) use reporting::{DateScope, TimeWindow};
pub(crate) use timeline_validation::{TimelineValidationError, validate_records};
pub(crate) use timesheet::{
    DailyTotals, TimeCategory, Timesheet, WeeklyTotals, calculate_time_breakdown,
};
pub(crate) use work::{WorkCommand, WorkContext, WorkRecord, WorkRecordInput};
pub(crate) use work_state::{TransitionReason, transition_day_state};
