#[cfg(test)]
mod tests {
    use super::super::{WorkCommand, WorkContext, WorkRecord};
    use crate::test_support::*;

    use chrono::Local;

    fn entry(id: u64, ts: chrono::DateTime<Local>, cmd: WorkCommand) -> WorkRecord {
        WorkRecord::new(id.into(), ts, cmd)
    }

    fn make_entry(id: u64, mins: u32, cmd: WorkCommand) -> WorkRecord {
        entry(id, ts(10, 16, mins), cmd)
    }

    fn entries_at_same_day(cmds: &[WorkCommand]) -> Vec<WorkRecord> {
        cmds.iter()
            .enumerate()
            .map(|(i, cmd)| make_entry((i + 1) as u64, (i as u32) * 5, cmd.clone()))
            .collect()
    }

    fn refs(entries: &[WorkRecord]) -> Vec<&WorkRecord> {
        entries.iter().collect()
    }

    mod validate_day_entries_tests {
        use crate::domain::work_transition::{DayState, validate_day_records};

        use super::*;

        #[test]
        fn valid_full_day_returns_ok() {
            #[rustfmt::skip]
            let cmds = [
                WorkCommand::Start { context: WorkContext::Office },
                WorkCommand::Log { title: "doing stuff".into(), description: None },
                WorkCommand::BreakStart,
                WorkCommand::BreakEnd { description: None },
                WorkCommand::Log { title: "doing other stuff".into(), description: None },
                WorkCommand::End { title: "done with all stuff".into(), description: None },
            ];

            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            assert!(validate_day_records(&entries, &Default::default()).is_ok());
        }

        #[test]
        fn multiple_checkin_checkout_cycles_returns_ok() {
            #[rustfmt::skip]
            let cmds = [
                WorkCommand::Start { context: WorkContext::Office },
                WorkCommand::End { title: "id 2".into(), description: None },
                WorkCommand::Start { context: WorkContext::Office },
                WorkCommand::End { title: "id 4".into(), description: None },
                WorkCommand::Start { context: WorkContext::Office },
                WorkCommand::End { title: "id 6".into(), description: None },
            ];

            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            assert!(validate_day_records(&entries, &Default::default()).is_ok());
        }

        #[test]
        fn given_active_start_returns_error() {
            let cmds = [
                WorkCommand::Start { context: Default::default() },
                WorkCommand::Start { context: Default::default() },
            ];
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_records(&entries, &Default::default()).unwrap_err();
            assert_eq!(
                err.to_string(),
                "Day entries validation failed: Entry 2 - cannot start home while already active (home)"
            )
        }

        #[test]
        fn given_idle_end_returns_error_message() {
            let cmds = [WorkCommand::End { title: "".into(), description: None }];
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_records(&entries, &Default::default()).unwrap_err();
            assert_eq!(
                err.to_string(),
                "Day entries validation failed: Entry 1 - cannot end while idle"
            );
        }

        #[test]
        fn given_idle_log_returns_error_message() {
            let cmds = [WorkCommand::Log { title: "".into(), description: None }];
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_records(&entries, &Default::default()).unwrap_err();
            assert_eq!(
                err.to_string(),
                "Day entries validation failed: Entry 1 - cannot log while idle"
            )
        }

        #[test]
        fn given_idle_break_start_returns_error_message() {
            let cmds = [WorkCommand::BreakStart];
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_records(&entries, &Default::default()).unwrap_err();
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
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_records(&entries, &Default::default()).unwrap_err();
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
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_records(&entries, &Default::default()).unwrap_err();
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
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_records(&entries, &Default::default()).unwrap_err();
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
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_records(&entries, &Default::default()).unwrap_err();
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
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);
            let prev_state = DayState::Active(WorkContext::Office);

            let end_state = validate_day_records(&entries, &prev_state).unwrap();
            assert_eq!(end_state, DayState::Idle);
        }

        #[test]
        fn given_breaking_prev_state_allows_break_end_then_end() {
            let cmds = [
                WorkCommand::BreakEnd { description: None },
                WorkCommand::End { title: "wrap".into(), description: None },
            ];
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);
            let prev_state = DayState::Breaking(WorkContext::Office);

            let end_state = validate_day_records(&entries, &prev_state).unwrap();
            assert_eq!(end_state, DayState::Idle);
        }
    }
}
