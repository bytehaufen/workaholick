use std::fmt::{Display, Formatter};

use crate::domain::{RecordId, WorkCommand, WorkContext};

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum DayState {
    #[default]
    Idle,
    Active(WorkContext),
    Breaking(WorkContext),
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
pub(crate) enum TransitionReason {
    WhileIdle,
    AlreadyActive(WorkContext),
    NotBreaking,
    WhileBreaking,
    AlreadyBreaking,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DayRecordValidationIssue {
    InvalidTransition { record_id: RecordId, attempted: CommandLabel, reason: TransitionReason },
}

impl Display for DayRecordValidationIssue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DayRecordValidationIssue::InvalidTransition { record_id, attempted, reason } => {
                match reason {
                    TransitionReason::WhileIdle => {
                        write!(f, "Entry {record_id} - cannot {attempted} while idle")
                    }
                    TransitionReason::AlreadyActive(kind) => {
                        write!(
                            f,
                            "Entry {record_id} - cannot {attempted} while already active ({kind})"
                        )
                    }
                    TransitionReason::NotBreaking => {
                        write!(f, "Entry {record_id} - cannot {attempted} when not breaking")
                    }
                    TransitionReason::WhileBreaking => {
                        write!(f, "Entry {record_id} - cannot {attempted} while breaking")
                    }
                    TransitionReason::AlreadyBreaking => {
                        write!(f, "Entry {record_id} - cannot {attempted} while already breaking")
                    }
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Day entries validation failed: {issue}")]
pub(crate) struct DayRecordValidationError {
    issue: DayRecordValidationIssue,
}

impl DayRecordValidationError {
    pub fn new(issue: DayRecordValidationIssue) -> Self {
        Self { issue }
    }

    pub fn issue(&self) -> DayRecordValidationIssue {
        self.issue
    }
}

pub(crate) fn transition_day_state(
    state: DayState,
    record_id: RecordId,
    command: &WorkCommand,
) -> Result<DayState, DayRecordValidationError> {
    use DayState::*;

    match (state, command) {
        (Idle, WorkCommand::Start { context }) => Ok(Active(*context)),
        (Idle, command) => {
            Err(DayRecordValidationError::new(DayRecordValidationIssue::InvalidTransition {
                record_id,
                attempted: command.into(),
                reason: TransitionReason::WhileIdle,
            }))
        }

        (Active(_), WorkCommand::Log { .. }) => Ok(state),

        (Active(kind), WorkCommand::BreakStart) => Ok(Breaking(kind)),
        (Breaking(kind), WorkCommand::BreakEnd { .. }) => Ok(Active(kind)),

        (Active(_), WorkCommand::End { .. }) => Ok(Idle),

        (Active(prev_context), WorkCommand::Start { context }) => {
            Err(DayRecordValidationError::new(DayRecordValidationIssue::InvalidTransition {
                record_id,
                attempted: CommandLabel::Start(*context),
                reason: TransitionReason::AlreadyActive(prev_context),
            }))
        }

        (Active(_), WorkCommand::BreakEnd { .. }) => {
            Err(DayRecordValidationError::new(DayRecordValidationIssue::InvalidTransition {
                record_id,
                attempted: CommandLabel::BreakEnd,
                reason: TransitionReason::NotBreaking,
            }))
        }

        (
            Breaking(_),
            WorkCommand::Start { .. } | WorkCommand::End { .. } | WorkCommand::Log { .. },
        ) => Err(DayRecordValidationError::new(DayRecordValidationIssue::InvalidTransition {
            record_id,
            attempted: command.into(),
            reason: TransitionReason::WhileBreaking,
        })),

        (Breaking(_), WorkCommand::BreakStart) => {
            Err(DayRecordValidationError::new(DayRecordValidationIssue::InvalidTransition {
                record_id,
                attempted: CommandLabel::BreakStart,
                reason: TransitionReason::AlreadyBreaking,
            }))
        }
    }
}
