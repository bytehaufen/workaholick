use std::fmt::{Display, Formatter};

use crate::domain::{ActivityKind, Command, Entry, EntryId};

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub(super) enum State {
    #[default]
    Idle,
    Active(ActivityKind),
    Breaking(ActivityKind),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct DayEntryInfo {
    pub(super) ids: Vec<EntryId>,
    pub(super) end_state: State,
}

impl DayEntryInfo {
    pub(super) fn try_new(
        entries: &Vec<&Entry>,
        prev_state: &State,
    ) -> Result<Self, DayEntryValidationError> {
        if entries.is_empty() {
            return Err(DayEntryValidationError::new(DayEntryValidationIssue::EmptyEntries));
        }
        let first_date = entries[0].date();

        if !entries.iter().all(|entry| entry.date() == first_date) {
            return Err(DayEntryValidationError::new(DayEntryValidationIssue::MixedDates));
        }

        let end_state = validate_day_entries(entries, prev_state)?;

        let ids = entries.iter().map(|entry| *entry.id()).collect();
        Ok(Self { end_state, ids })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum CommandLabel {
    Start,
    StartWithKind(ActivityKind),
    End,
    Log,
    BreakStart,
    BreakEnd,
}

impl Display for CommandLabel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start => f.write_str("start"),
            Self::StartWithKind(kind) => f.write_str(&format!("start {kind}")),
            Self::End => f.write_str("end"),
            Self::Log => f.write_str("log"),
            Self::BreakStart => f.write_str("start break"),
            Self::BreakEnd => f.write_str("end break"),
        }
    }
}

impl From<&Command> for CommandLabel {
    fn from(command: &Command) -> Self {
        match command {
            Command::Start { .. } => Self::Start,
            Command::End { .. } => Self::End,
            Command::Log { .. } => Self::Log,
            Command::BreakStart => Self::BreakStart,
            Command::BreakEnd { .. } => Self::BreakEnd,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TransitionReason {
    WhileIdle,
    AlreadyActive(ActivityKind),
    NotBreaking,
    WhileBreaking,
    AlreadyBreaking,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DayEntryValidationIssue {
    EmptyEntries,
    MixedDates,
    InvalidTransition { entry_id: EntryId, attempted: CommandLabel, reason: TransitionReason },
}

impl Display for DayEntryValidationIssue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DayEntryValidationIssue::EmptyEntries => f.write_str("entries must not be empty"),
            DayEntryValidationIssue::MixedDates => {
                f.write_str("entries must all be of the same date")
            }
            DayEntryValidationIssue::InvalidTransition { entry_id, attempted, reason } => {
                match reason {
                    TransitionReason::WhileIdle => {
                        write!(f, "Entry {entry_id} - cannot {attempted} while idle")
                    }
                    TransitionReason::AlreadyActive(kind) => {
                        write!(
                            f,
                            "Entry {entry_id} - cannot {attempted} while already active ({kind})"
                        )
                    }
                    TransitionReason::NotBreaking => {
                        write!(f, "Entry {entry_id} - cannot {attempted} when not breaking")
                    }
                    TransitionReason::WhileBreaking => {
                        write!(f, "Entry {entry_id} - cannot {attempted} while breaking")
                    }
                    TransitionReason::AlreadyBreaking => {
                        write!(f, "Entry {entry_id} - cannot {attempted} while already breaking")
                    }
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Day entries validation failed: {issue}")]
pub(crate) struct DayEntryValidationError {
    issue: DayEntryValidationIssue,
}

impl DayEntryValidationError {
    fn new(issue: DayEntryValidationIssue) -> Self {
        Self { issue }
    }

    pub(crate) fn issue(&self) -> DayEntryValidationIssue {
        self.issue
    }
}

pub(super) fn validate_day_entries(
    entries: &[&Entry],
    prev_state: &State,
) -> Result<State, DayEntryValidationError> {
    use State::*;

    entries.iter().try_fold(*prev_state, |state, entry| match (state, entry.command()) {
        (Idle, Command::Start { kind, .. }) => Ok(Active(*kind)),
        (Idle, command) => {
            Err(DayEntryValidationError::new(DayEntryValidationIssue::InvalidTransition {
                entry_id: *entry.id(),
                attempted: command.into(),
                reason: TransitionReason::WhileIdle,
            }))
        }

        (Active(_), Command::Log { .. }) => Ok(state),

        (Active(kind), Command::BreakStart) => Ok(Breaking(kind)),
        (Breaking(kind), Command::BreakEnd { .. }) => Ok(Active(kind)),

        (Active(_), Command::End { .. }) => Ok(Idle),

        (Active(activity_kind), Command::Start { kind }) => {
            Err(DayEntryValidationError::new(DayEntryValidationIssue::InvalidTransition {
                entry_id: *entry.id(),
                attempted: CommandLabel::StartWithKind(*kind),
                reason: TransitionReason::AlreadyActive(activity_kind),
            }))
        }

        (Active(_), Command::BreakEnd { .. }) => {
            Err(DayEntryValidationError::new(DayEntryValidationIssue::InvalidTransition {
                entry_id: *entry.id(),
                attempted: CommandLabel::BreakEnd,
                reason: TransitionReason::NotBreaking,
            }))
        }

        (Breaking(_), Command::Start { .. } | Command::End { .. } | Command::Log { .. }) => {
            Err(DayEntryValidationError::new(DayEntryValidationIssue::InvalidTransition {
                entry_id: *entry.id(),
                attempted: entry.command().into(),
                reason: TransitionReason::WhileBreaking,
            }))
        }

        (Breaking(_), Command::BreakStart) => {
            Err(DayEntryValidationError::new(DayEntryValidationIssue::InvalidTransition {
                entry_id: *entry.id(),
                attempted: CommandLabel::BreakStart,
                reason: TransitionReason::AlreadyBreaking,
            }))
        }
    })
}
