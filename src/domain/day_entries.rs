#[cfg(test)]
mod tests {
    use super::super::{WorkCommand, WorkContext, WorkRecord};
    use crate::test_support::*;

    use chrono::{Local, TimeZone};

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

    mod day_entries {
        use crate::domain::work_transition::DayRecordInfo;

        use super::*;

        #[test]
        fn try_from_empty_returns_error() {
            let err = DayRecordInfo::try_new(&Vec::new(), &Default::default()).unwrap_err();
            assert_eq!(err.to_string(), "Day entries validation failed: entries must not be empty");
        }

        #[test]
        fn try_from_mixed_dates_returns_error() {
            let entry1 = entry(1, ts(4, 16, 8), WorkCommand::default());
            let entry2 = entry(
                2,
                Local.with_ymd_and_hms(4025, 1, 4, 16, 9, 0).unwrap(),
                WorkCommand::default(),
            );

            let err =
                DayRecordInfo::try_new(&vec![&entry1, &entry2], &Default::default()).unwrap_err();
            assert_eq!(
                err.to_string(),
                "Day entries validation failed: entries must all be of the same date"
            );
        }
    }

    mod index_entries_by_date_tests {
        use crate::domain::work_timeline_validation::validate_work_timeline;

        use super::*;

        #[test]
        fn returns_empty_map_for_empty_input() {
            let input: Vec<&WorkRecord> = Vec::new();
            let by_date = validate_work_timeline(&input).unwrap();
            assert!(by_date.is_empty());
        }

        #[test]
        fn partitions_entries_into_multiple_days() {
            let entry1 = entry(
                1,
                Local.with_ymd_and_hms(2026, 1, 4, 8, 0, 0).unwrap(),
                WorkCommand::Start { context: WorkContext::Office },
            );
            let entry2 = entry(
                2,
                Local.with_ymd_and_hms(2026, 1, 4, 9, 0, 0).unwrap(),
                WorkCommand::BreakStart,
            );
            let entry3 = entry(
                3,
                Local.with_ymd_and_hms(2026, 1, 4, 9, 30, 0).unwrap(),
                WorkCommand::BreakEnd { description: None },
            );
            let entry4 = entry(
                4,
                Local.with_ymd_and_hms(2026, 1, 4, 10, 0, 0).unwrap(),
                WorkCommand::End { title: "".into(), description: None },
            );
            let entry5 = entry(
                5,
                Local.with_ymd_and_hms(2026, 1, 5, 10, 0, 0).unwrap(),
                WorkCommand::Start { context: WorkContext::Office },
            );

            let entries: Vec<&WorkRecord> = vec![&entry1, &entry2, &entry3, &entry4, &entry5];
            let by_date = validate_work_timeline(&entries).unwrap();

            assert_eq!(by_date.len(), 2);
            assert!(by_date.contains_key(&entry1.timestamp().date_naive()));
            assert!(by_date.contains_key(&entry3.timestamp().date_naive()));

            let day1 = by_date.get(&entry1.timestamp().date_naive()).unwrap();
            let day1_timestamps: Vec<_> = day1
                .ids
                .iter()
                .map(|id| *entries.iter().find(|entry| entry.id() == id).unwrap().timestamp())
                .collect();
            assert_eq!(
                day1_timestamps,
                vec![
                    *entry1.timestamp(),
                    *entry2.timestamp(),
                    *entry3.timestamp(),
                    *entry4.timestamp()
                ]
            );

            let day2 = by_date.get(&entry5.timestamp().date_naive()).unwrap();
            let day2_timestamps: Vec<_> = day2
                .ids
                .iter()
                .map(|id| *entries.iter().find(|entry| entry.id() == id).unwrap().timestamp())
                .collect();
            assert_eq!(day2_timestamps, vec![*entry5.timestamp()]);
        }
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
