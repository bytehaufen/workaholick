use std::fmt::{Display, Formatter};

use crate::domain::{RecordId, WorkCommand, WorkContext, WorkRecord};

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
    EmptyRecords,
    MixedDates,
    InvalidTransition { record_id: RecordId, attempted: CommandLabel, reason: TransitionReason },
}

impl Display for DayRecordValidationIssue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DayRecordValidationIssue::EmptyRecords => f.write_str("entries must not be empty"),
            DayRecordValidationIssue::MixedDates => {
                f.write_str("entries must all be of the same date")
            }
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

pub(super) fn validate_day_records(
    records: &[&WorkRecord],
    prev_state: &DayState,
) -> Result<DayState, DayRecordValidationError> {
    records.iter().try_fold(*prev_state, |state, record| {
        transition_day_state(state, *record.id(), record.command())
    })
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

#[cfg(test)]
mod tests {
    use super::super::{WorkCommand, WorkContext, WorkRecord};
    use crate::test_support::*;

    use chrono::Local;

    fn record(id: u64, ts: chrono::DateTime<Local>, cmd: WorkCommand) -> WorkRecord {
        WorkRecord::new(id.into(), ts, cmd)
    }

    fn make_record(id: u64, mins: u32, cmd: WorkCommand) -> WorkRecord {
        record(id, ts(10, 16, mins), cmd)
    }

    fn records_at_same_day(cmds: &[WorkCommand]) -> Vec<WorkRecord> {
        cmds.iter()
            .enumerate()
            .map(|(i, cmd)| make_record((i + 1) as u64, (i as u32) * 5, cmd.clone()))
            .collect()
    }

    fn refs(records: &[WorkRecord]) -> Vec<&WorkRecord> {
        records.iter().collect()
    }

    mod validate_day_records_tests {
        use crate::domain::work_transition::{DayState, validate_day_records};

        use super::*;

        #[test]
        fn valid_full_day_returns_ok() {
            let cmds = [
                WorkCommand::Start { context: WorkContext::Office },
                WorkCommand::Log { title: "doing stuff".into(), description: None },
                WorkCommand::BreakStart,
                WorkCommand::BreakEnd { description: None },
                WorkCommand::Log { title: "doing other stuff".into(), description: None },
                WorkCommand::End { title: "done with all stuff".into(), description: None },
            ];

            let records = records_at_same_day(&cmds);
            let records = refs(&records);

            assert!(validate_day_records(&records, &Default::default()).is_ok());
        }

        #[test]
        fn multiple_checkin_checkout_cycles_returns_ok() {
            let cmds = [
                WorkCommand::Start { context: WorkContext::Office },
                WorkCommand::End { title: "id 2".into(), description: None },
                WorkCommand::Start { context: WorkContext::Office },
                WorkCommand::End { title: "id 4".into(), description: None },
                WorkCommand::Start { context: WorkContext::Office },
                WorkCommand::End { title: "id 6".into(), description: None },
            ];

            let records = records_at_same_day(&cmds);
            let records = refs(&records);

            assert!(validate_day_records(&records, &Default::default()).is_ok());
        }

        #[test]
        fn given_active_start_returns_error() {
            let cmds = [
                WorkCommand::Start { context: Default::default() },
                WorkCommand::Start { context: Default::default() },
            ];
            let records = records_at_same_day(&cmds);
            let records = refs(&records);

            let err = validate_day_records(&records, &Default::default()).unwrap_err();
            assert_eq!(
                err.to_string(),
                "Day entries validation failed: Entry 2 - cannot start home while already active (home)"
            )
        }

        #[test]
        fn given_idle_end_returns_error_message() {
            let cmds = [WorkCommand::End { title: "".into(), description: None }];
            let records = records_at_same_day(&cmds);
            let records = refs(&records);

            let err = validate_day_records(&records, &Default::default()).unwrap_err();
            assert_eq!(
                err.to_string(),
                "Day entries validation failed: Entry 1 - cannot end while idle"
            );
        }

        #[test]
        fn given_idle_log_returns_error_message() {
            let cmds = [WorkCommand::Log { title: "".into(), description: None }];
            let records = records_at_same_day(&cmds);
            let records = refs(&records);

            let err = validate_day_records(&records, &Default::default()).unwrap_err();
            assert_eq!(
                err.to_string(),
                "Day entries validation failed: Entry 1 - cannot log while idle"
            )
        }

        #[test]
        fn given_idle_break_start_returns_error_message() {
            let cmds = [WorkCommand::BreakStart];
            let records = records_at_same_day(&cmds);
            let records = refs(&records);

            let err = validate_day_records(&records, &Default::default()).unwrap_err();
            assert_eq!(
                err.to_string(),
                "Day entries validation failed: Entry 1 - cannot start break while idle"
            );
        }

        #[test]
        fn given_active_break_end_returns_error_message() {
            let cmds = [
                WorkCommand::Start { context: Default::default() },
                WorkCommand::BreakEnd { description: None },
            ];
            let records = records_at_same_day(&cmds);
            let records = refs(&records);

            let err = validate_day_records(&records, &Default::default()).unwrap_err();
            assert_eq!(
                err.to_string(),
                "Day entries validation failed: Entry 2 - cannot end break when not breaking"
            );
        }

        #[test]
        fn given_breaking_end_returns_error_message() {
            let cmds = [
                WorkCommand::Start { context: Default::default() },
                WorkCommand::BreakStart,
                WorkCommand::End { title: "".into(), description: None },
            ];
            let records = records_at_same_day(&cmds);
            let records = refs(&records);

            let err = validate_day_records(&records, &Default::default()).unwrap_err();
            assert_eq!(
                err.to_string(),
                "Day entries validation failed: Entry 3 - cannot end while breaking"
            );
        }

        #[test]
        fn given_breaking_log_returns_error_message() {
            let cmds = [
                WorkCommand::Start { context: Default::default() },
                WorkCommand::BreakStart,
                WorkCommand::Log { title: "".into(), description: None },
            ];
            let records = records_at_same_day(&cmds);
            let records = refs(&records);

            let err = validate_day_records(&records, &Default::default()).unwrap_err();
            assert_eq!(
                err.to_string(),
                "Day entries validation failed: Entry 3 - cannot log while breaking"
            );
        }

        #[test]
        fn given_breaking_break_start_returns_error_message() {
            let cmds = [
                WorkCommand::Start { context: Default::default() },
                WorkCommand::BreakStart,
                WorkCommand::BreakStart,
            ];
            let records = records_at_same_day(&cmds);
            let records = refs(&records);

            let err = validate_day_records(&records, &Default::default()).unwrap_err();
            assert_eq!(
                err.to_string(),
                "Day entries validation failed: Entry 3 - cannot start break while already breaking"
            );
        }

        #[test]
        fn given_active_prev_state_allows_log_then_end() {
            let cmds = [
                WorkCommand::Log { title: "handoff".into(), description: None },
                WorkCommand::End { title: "wrap".into(), description: None },
            ];
            let records = records_at_same_day(&cmds);
            let records = refs(&records);
            let prev_state = DayState::Active(WorkContext::Office);

            let end_state = validate_day_records(&records, &prev_state).unwrap();
            assert_eq!(end_state, DayState::Idle);
        }

        #[test]
        fn given_breaking_prev_state_allows_break_end_then_end() {
            let cmds = [
                WorkCommand::BreakEnd { description: None },
                WorkCommand::End { title: "wrap".into(), description: None },
            ];
            let records = records_at_same_day(&cmds);
            let records = refs(&records);
            let prev_state = DayState::Breaking(WorkContext::Office);

            let end_state = validate_day_records(&records, &prev_state).unwrap();
            assert_eq!(end_state, DayState::Idle);
        }
    }
}
