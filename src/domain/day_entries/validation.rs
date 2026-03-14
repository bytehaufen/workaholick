use colored::Colorize;

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
            return Err(DayEntryValidationError("entries must not be empty".into()));
        }
        let first_date = entries[0].date();

        if !entries.iter().all(|entry| entry.date() == first_date) {
            return Err(DayEntryValidationError("entries must all be of the same date".into()));
        }

        let end_state = validate_day_entries(entries, prev_state)?;

        let ids = entries.iter().map(|entry| *entry.id()).collect();
        Ok(Self { end_state, ids })
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Day entries validation failed: {0}")]
pub(super) struct DayEntryValidationError(pub(super) String);

pub(super) fn validate_day_entries(
    entries: &[&Entry],
    prev_state: &State,
) -> Result<State, DayEntryValidationError> {
    use State::*;

    entries.iter().try_fold(*prev_state, |state, entry| match (state, entry.command()) {
        (Idle, Command::Start { kind, .. }) => Ok(Active(*kind)),
        (Idle, command) => Err(DayEntryValidationError(format!(
            "Entry {} - cannot {} while {}",
            entry.id(),
            command.to_string().yellow(),
            "idle".yellow()
        ))),

        (Active(_), Command::Log { .. }) => Ok(state),

        (Active(kind), Command::BreakStart) => Ok(Breaking(kind)),
        (Breaking(kind), Command::BreakEnd { .. }) => Ok(Active(kind)),

        (Active(_), Command::End { .. }) => Ok(Idle),

        (Active(activity_kind), Command::Start { kind }) => Err(DayEntryValidationError(format!(
            "Entry {} - cannot {} while already active ({})",
            entry.id(),
            format!("start {kind}").yellow(),
            activity_kind.to_string().yellow()
        ))),

        (Active(_), Command::BreakEnd { .. }) => Err(DayEntryValidationError(format!(
            "Entry {} - cannot {} when {}",
            entry.id(),
            "end break".yellow(),
            "not breaking".yellow()
        ))),

        (Breaking(_), Command::Start { .. } | Command::End { .. } | Command::Log { .. }) => {
            Err(DayEntryValidationError(format!(
                "Entry {} - cannot {} while {}",
                entry.id(),
                entry.command().to_string().yellow(),
                "breaking".yellow()
            )))
        }

        (Breaking(_), Command::BreakStart) => Err(DayEntryValidationError(format!(
            "Entry {} - cannot {} while already {}",
            entry.id(),
            "start break".yellow(),
            "breaking".yellow()
        ))),
    })
}
