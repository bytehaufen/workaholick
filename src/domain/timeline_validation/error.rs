use std::fmt::{Display, Formatter};

use chrono::NaiveDate;

use crate::domain::{TransitionReason, WorkCommand, WorkContext};

#[derive(Debug, thiserror::Error)]
#[error("{issue} for {date}")]
pub(crate) struct TimelineValidationError {
    date: NaiveDate,
    issue: TimelineValidationIssue,
}

impl TimelineValidationError {
    pub(crate) fn work(date: NaiveDate, issue: WorkValidationIssue) -> Self {
        Self { date, issue: TimelineValidationIssue::Work(issue) }
    }

    pub(crate) fn absence(date: NaiveDate, issue: AbsenceValidationIssue) -> Self {
        Self { date, issue: TimelineValidationIssue::Absence(issue) }
    }

    pub(crate) fn cross(date: NaiveDate, issue: CrossValidationIssue) -> Self {
        Self { date, issue: TimelineValidationIssue::Cross(issue) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub(crate) enum TimelineValidationIssue {
    #[error("{0}")]
    Work(WorkValidationIssue),
    #[error("{0}")]
    Absence(AbsenceValidationIssue),
    #[error("{0}")]
    Cross(CrossValidationIssue),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum CommandLabel {
    Start(WorkContext),
    End,
    Log,
    BreakStart,
    BreakEnd,
}

impl Display for CommandLabel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start(context) => f.write_str(&format!("start {context}")),
            Self::End => f.write_str("end"),
            Self::Log => f.write_str("log"),
            Self::BreakStart => f.write_str("start break"),
            Self::BreakEnd => f.write_str("end break"),
        }
    }
}

impl From<&WorkCommand> for CommandLabel {
    fn from(command: &WorkCommand) -> Self {
        match command {
            WorkCommand::Start { context } => Self::Start(*context),
            WorkCommand::End { .. } => Self::End,
            WorkCommand::Log { .. } => Self::Log,
            WorkCommand::BreakStart => Self::BreakStart,
            WorkCommand::BreakEnd { .. } => Self::BreakEnd,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WorkValidationIssue {
    attempted: CommandLabel,
    reason: TransitionReason,
}

impl WorkValidationIssue {
    pub(crate) fn new(attempted: CommandLabel, reason: TransitionReason) -> Self {
        Self { attempted, reason }
    }
}

impl Display for WorkValidationIssue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { attempted, reason } = self;

        match reason {
            TransitionReason::WhileIdle => {
                write!(f, "cannot {attempted} while idle")
            }
            TransitionReason::AlreadyActive(kind) => {
                write!(f, "cannot {attempted} while already active ({kind})")
            }
            TransitionReason::NotBreaking => {
                write!(f, "cannot {attempted} when not breaking")
            }
            TransitionReason::WhileBreaking => {
                write!(f, "cannot {attempted} while breaking")
            }
            TransitionReason::AlreadyBreaking => {
                write!(f, "cannot {attempted} while already breaking")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub(crate) enum AbsenceValidationIssue {
    #[error("full-day absence cannot be combined with half-day absences")]
    FullDayWithHalfDay,
    #[error("more than one full-day absence")]
    MultipleFullDays,
    #[error("more than two half-day absences")]
    MoreThanTwoHalfDays,
}

#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub(crate) enum CrossValidationIssue {
    #[error("full-day absence cannot coexist with work records")]
    FullDayAbsenceWithWork,
    #[error("two half-day absences cannot coexist with work records")]
    TwoHalfDayAbsenceWithWork,
}
