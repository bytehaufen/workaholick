use chrono::NaiveDate;

use crate::domain::{reporting::ReportingError, time_accounting::TimeAccountingError};

#[derive(Debug, thiserror::Error)]
pub(crate) enum TimesheetError {
    #[error(transparent)]
    Reporting(#[from] ReportingError),

    #[error(transparent)]
    Accounting(#[from] TimeAccountingError),

    #[error("cannot calculate the end of the week starting on {week_start}")]
    WeekEndOutOfRange { week_start: NaiveDate },
}
