use chrono::NaiveDate;

use crate::domain::{
    WorkCommand, WorkContext,
    timeline_validation::{CommandLabel, TimelineValidationError, WorkValidationIssue},
};

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum DayState {
    #[default]
    Idle,
    Active(WorkContext),
    Breaking(WorkContext),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TransitionReason {
    WhileIdle,
    AlreadyActive(WorkContext),
    NotBreaking,
    WhileBreaking,
    AlreadyBreaking,
}

pub(crate) fn transition_day_state(
    state: DayState,
    date: NaiveDate,
    command: &WorkCommand,
) -> Result<DayState, TimelineValidationError> {
    use DayState::*;

    match (state, command) {
        (Idle, WorkCommand::Start { context }) => Ok(Active(*context)),
        (Idle, command) => Err(TimelineValidationError::work(
            date,
            WorkValidationIssue::new(command.into(), TransitionReason::WhileIdle),
        )),

        (Active(_), WorkCommand::Log { .. }) => Ok(state),

        (Active(kind), WorkCommand::BreakStart) => Ok(Breaking(kind)),
        (Breaking(kind), WorkCommand::BreakEnd { .. }) => Ok(Active(kind)),

        (Active(_), WorkCommand::End { .. }) => Ok(Idle),

        (Active(prev_context), WorkCommand::Start { context }) => {
            Err(TimelineValidationError::work(
                date,
                WorkValidationIssue::new(
                    CommandLabel::Start(*context),
                    TransitionReason::AlreadyActive(prev_context),
                ),
            ))
        }

        (Active(_), WorkCommand::BreakEnd { .. }) => Err(TimelineValidationError::work(
            date,
            WorkValidationIssue::new(CommandLabel::BreakEnd, TransitionReason::NotBreaking),
        )),

        (
            Breaking(_),
            WorkCommand::Start { .. } | WorkCommand::End { .. } | WorkCommand::Log { .. },
        ) => Err(TimelineValidationError::work(
            date,
            WorkValidationIssue::new(command.into(), TransitionReason::WhileBreaking),
        )),

        (Breaking(_), WorkCommand::BreakStart) => Err(TimelineValidationError::work(
            date,
            WorkValidationIssue::new(CommandLabel::BreakStart, TransitionReason::AlreadyBreaking),
        )),
    }
}
