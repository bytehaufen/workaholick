mod day_state;
mod index;
mod validation;

pub(crate) use day_state::DayState;
pub(crate) use index::WorkRecordValidationIndex;
pub(crate) use validation::{
    CommandLabel, DayRecordValidationError, DayRecordValidationIssue, TransitionReason,
    transition_day_state,
};

#[cfg(test)]
mod tests {
    use super::super::{RecordId, WorkCommand, WorkContext, WorkRecord, WorkRecordInput};
    use super::*;
    use index::index_records_by_date;
    use validation::{DayRecordInfo, validate_day_records};

    use chrono::{Local, NaiveDate, TimeZone};
    use std::collections::BTreeMap;

    fn ts(day: u32, hour: u32, min: u32) -> chrono::DateTime<Local> {
        Local.with_ymd_and_hms(2025, 1, day, hour, min, 0).unwrap()
    }

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

    fn id_refs(entries: &[WorkRecord]) -> Vec<&WorkRecord> {
        entries.iter().collect()
    }

    mod day_entries {
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
        use super::*;

        #[test]
        fn returns_empty_map_for_empty_input() {
            let input: Vec<&WorkRecord> = Vec::new();
            let by_date = index_records_by_date(&input).unwrap();
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
            let by_date = index_records_by_date(&entries).unwrap();

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

    mod entries_by_day_tests {
        use super::*;

        #[test]
        fn try_new_rejects_start_when_previous_day_is_active() {
            let day1 = [entry(
                1,
                Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                WorkCommand::Start { context: Default::default() },
            )];
            let day2 = [
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);

            let err = WorkRecordValidationIndex::try_new(&entries).unwrap_err().to_string();
            assert_eq!(
                err.to_string(),
                "Day entries validation failed: Entry 2 - cannot start home while already active (home)"
            );
        }

        #[test]
        fn try_new_allows_spanning_open_sessions_over_day_boundary() {
            let day1 = vec![entry(
                0,
                Local.with_ymd_and_hms(2026, 1, 11, 5, 4, 19).unwrap(),
                WorkCommand::Start { context: Default::default() },
            )];
            let day2 = vec![
                entry(
                    1,
                    Local.with_ymd_and_hms(2026, 1, 14, 20, 39, 15).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2026, 1, 14, 21, 9, 40).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    3,
                    Local.with_ymd_and_hms(2026, 1, 14, 21, 9, 47).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];
            let day3 = vec![entry(
                4,
                Local.with_ymd_and_hms(2026, 1, 18, 6, 4, 54).unwrap(),
                WorkCommand::Start { context: Default::default() },
            )];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);
            all.extend_from_slice(&day3);
            let entries = id_refs(&all);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            let mut expected: BTreeMap<NaiveDate, DayRecordInfo> = BTreeMap::new();
            let day_entries_1 = DayRecordInfo {
                ids: day1.iter().map(|e| *e.id()).collect(),
                end_state: DayState::Active(Default::default()),
            };
            let day_entries_2 = DayRecordInfo {
                ids: day2.iter().map(|e| *e.id()).collect(),
                end_state: DayState::Idle,
            };
            let day_entries_3 = DayRecordInfo {
                ids: day3.iter().map(|e| *e.id()).collect(),
                end_state: DayState::Active(Default::default()),
            };
            expected.insert(NaiveDate::from_ymd_opt(2026, 1, 11).unwrap(), day_entries_1);
            expected.insert(NaiveDate::from_ymd_opt(2026, 1, 14).unwrap(), day_entries_2);
            expected.insert(NaiveDate::from_ymd_opt(2026, 1, 18).unwrap(), day_entries_3);
            assert_eq!(by_day.days, expected);
        }

        #[test]
        fn try_new_accepts_valid_days() {
            let ts1 = Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap();
            let ts2 = Local.with_ymd_and_hms(2025, 1, 10, 10, 0, 0).unwrap();
            let ts3 = Local.with_ymd_and_hms(2025, 1, 10, 11, 0, 0).unwrap();
            let day1 = [
                entry(1, ts1, WorkCommand::Start { context: Default::default() }),
                entry(2, ts2, WorkCommand::Log { title: "".into(), description: None }),
                entry(3, ts3, WorkCommand::End { title: "".into(), description: None }),
            ];

            let ts4 = Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap();
            let ts5 = Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap();
            let day2 = [
                entry(4, ts4, WorkCommand::Start { context: Default::default() }),
                entry(5, ts5, WorkCommand::Log { title: "".into(), description: None }),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            let mut expected: BTreeMap<NaiveDate, DayRecordInfo> = BTreeMap::new();
            let day_entries_1 = DayRecordInfo {
                ids: day1.iter().map(|e| *e.id()).collect(),
                end_state: DayState::Idle,
            };
            let day_entries_2 = DayRecordInfo {
                ids: day2.iter().map(|e| *e.id()).collect(),
                end_state: DayState::Active(Default::default()),
            };
            expected.insert(ts1.date_naive(), day_entries_1);
            expected.insert(ts4.date_naive(), day_entries_2);
            let expected = WorkRecordValidationIndex { days: expected };

            assert_eq!(by_day, expected);
        }

        #[test]
        fn validate_insert_entry_rejects_when_it_breaks_previous_day_boundary() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 10, 0, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            let patch = WorkRecordInput::new(
                Local.with_ymd_and_hms(2025, 1, 10, 11, 0, 0).unwrap(),
                WorkCommand::Log { title: "late log".into(), description: None },
            );

            let new_id: RecordId = 5.into();
            let err =
                by_day.validate_insert_record(&entries, &patch, new_id).unwrap_err().to_string();

            assert_eq!(err, "Day entries validation failed: Entry 5 - cannot log while idle");
        }

        #[test]
        fn validate_insert_entry_allows_valid_insert_in_same_day() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 10, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            let patch = WorkRecordInput::new(
                Local.with_ymd_and_hms(2025, 1, 11, 9, 2, 0).unwrap(),
                WorkCommand::Log { title: "note".into(), description: None },
            );

            let new_id: RecordId = 5.into();
            assert!(by_day.validate_insert_record(&entries, &patch, new_id).is_ok());
        }

        #[test]
        fn validate_insert_entry_allows_future_day_when_existing_day_depends_on_earlier_gap_state()
        {
            let day1 = [
                entry(
                    4,
                    Local.with_ymd_and_hms(2026, 1, 18, 6, 4, 54).unwrap(),
                    WorkCommand::Start { context: WorkContext::Home },
                ),
                entry(
                    5,
                    Local.with_ymd_and_hms(2026, 1, 18, 7, 13, 39).unwrap(),
                    WorkCommand::BreakStart,
                ),
            ];
            let day2 = [
                entry(
                    6,
                    Local.with_ymd_and_hms(2026, 3, 29, 7, 13, 40).unwrap(),
                    WorkCommand::BreakEnd { description: None },
                ),
                entry(
                    7,
                    Local.with_ymd_and_hms(2026, 3, 29, 7, 13, 41).unwrap(),
                    WorkCommand::Log { title: "first".into(), description: Some("first".into()) },
                ),
                entry(
                    8,
                    Local.with_ymd_and_hms(2026, 3, 29, 7, 13, 42).unwrap(),
                    WorkCommand::End { title: "second".into(), description: Some("second".into()) },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            let patch = WorkRecordInput::new(
                Local.with_ymd_and_hms(2026, 4, 6, 9, 0, 0).unwrap(),
                WorkCommand::Start { context: WorkContext::Home },
            );

            assert!(by_day.validate_insert_record(&entries, &patch, 9.into()).is_ok());
        }

        #[test]
        fn validate_remove_entry_rejects_when_removal_makes_previous_day_invalid_for_next_day() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 5, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            let err =
                by_day.validate_remove_record(&entries, day1[1].id()).unwrap_err().to_string();
            assert_eq!(
                err,
                "Day entries validation failed: Entry 3 - cannot start home while already active (home)"
            );
        }

        #[test]
        fn validate_remove_entry_rejects_when_actual_day_gets_invalid() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 5, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            let err =
                by_day.validate_remove_record(&entries, day2[0].id()).unwrap_err().to_string();
            assert!(err.contains("cannot end while idle"));
        }

        #[test]
        fn validate_remove_entry_allows_removing_last_entry_and_prunes_day_on_apply() {
            let only = [entry(
                1,
                Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                WorkCommand::Start { context: Default::default() },
            )];

            let entries = id_refs(&only);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            by_day.validate_remove_record(&entries, only[0].id()).unwrap();

            assert!(by_day.validate_remove_record(&entries, only[0].id()).is_ok());
        }

        #[test]
        fn validate_remove_entry_allows_removing_valid() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 5, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 3, 0).unwrap(),
                    WorkCommand::Log { title: "".into(), description: None },
                ),
                entry(
                    5,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];
            let day3 = [
                entry(
                    6,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    7,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 5, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);
            all.extend_from_slice(&day3);

            let entries = id_refs(&all);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            by_day.validate_remove_record(&entries, day2[1].id()).expect("expect valid entries");
        }

        #[test]
        fn validate_update_entry_allows_reordering_on_different_days() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    WorkCommand::Log { title: "a".into(), description: None },
                ),
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 10, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    5,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 5, 0).unwrap(),
                    WorkCommand::Log { title: "a".into(), description: None },
                ),
                entry(
                    6,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 10, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];
            let day3 = [
                entry(
                    7,
                    Local.with_ymd_and_hms(2025, 1, 13, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    8,
                    Local.with_ymd_and_hms(2025, 1, 13, 9, 5, 0).unwrap(),
                    WorkCommand::Log { title: "a".into(), description: None },
                ),
                entry(
                    9,
                    Local.with_ymd_and_hms(2025, 1, 13, 9, 10, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];
            let days: Vec<_> = vec![day1, day2.clone(), day3].into_iter().flatten().collect();

            let entries = id_refs(&days);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            let patch = WorkRecordInput::new(
                Local.with_ymd_and_hms(2025, 1, 11, 9, 2, 0).unwrap(),
                WorkCommand::Log { title: "moved".into(), description: None },
            );

            let result = by_day.validate_update_record(&entries, day2[1].id(), &patch);

            assert!(result.is_ok(), "expected Ok, but was Err: {:?}", result.unwrap_err());
        }

        #[test]
        fn validate_update_entry_allows_reordering_on_same_day() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    WorkCommand::Log { title: "a".into(), description: None },
                ),
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 10, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    5,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 5, 0).unwrap(),
                    WorkCommand::Log { title: "a".into(), description: None },
                ),
                entry(
                    6,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 10, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];
            let day3 = [
                entry(
                    7,
                    Local.with_ymd_and_hms(2025, 1, 13, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    8,
                    Local.with_ymd_and_hms(2025, 1, 13, 9, 5, 0).unwrap(),
                    WorkCommand::Log { title: "a".into(), description: None },
                ),
                entry(
                    9,
                    Local.with_ymd_and_hms(2025, 1, 13, 9, 10, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];
            let days: Vec<_> = vec![day1, day2.clone(), day3].into_iter().flatten().collect();

            let entries = id_refs(&days);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            let patch = WorkRecordInput::new(
                Local.with_ymd_and_hms(2025, 1, 12, 9, 2, 0).unwrap(),
                WorkCommand::Log { title: "moved".into(), description: None },
            );

            let result = by_day.validate_update_record(&entries, day2[1].id(), &patch);

            assert!(result.is_ok(), "expected Ok, but was Err: {:?}", result.unwrap_err());
        }

        #[test]
        fn validate_update_entry_rejects_invalid_same_day_command_change() {
            let day = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    WorkCommand::Log { title: "a".into(), description: None },
                ),
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 10, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];

            let entries = id_refs(&day);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            let patch = WorkRecordInput::new(
                Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                WorkCommand::End { title: "nope".into(), description: None },
            );

            let err = by_day
                .validate_update_record(&entries, day[1].id(), &patch)
                .unwrap_err()
                .to_string();
            assert!(err.contains("cannot end while idle"));
        }

        #[test]
        fn validate_update_entry_rejects_invalid_same_day_reordering() {
            let day = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    WorkCommand::Log { title: "a".into(), description: None },
                ),
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 10, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];

            let entries = id_refs(&day);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            let patch = WorkRecordInput::new(
                Local.with_ymd_and_hms(2025, 1, 11, 8, 55, 0).unwrap(),
                WorkCommand::End { title: "".into(), description: None },
            );

            let err = by_day
                .validate_update_record(&entries, day[2].id(), &patch)
                .unwrap_err()
                .to_string();
            assert!(err.contains("cannot end while idle"));
        }

        #[test]
        fn validate_update_entry_allows_move_to_new_day_when_old_day_becomes_empty() {
            let only = [entry(
                1,
                Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                WorkCommand::Start { context: Default::default() },
            )];

            let entries = id_refs(&only);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            let patch = WorkRecordInput::new(
                Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                WorkCommand::Start { context: Default::default() },
            );

            let result = by_day.validate_update_record(&entries, only[0].id(), &patch);
            assert!(result.is_ok(), "expected Ok, but was Err: {:?}", result.unwrap_err());
        }

        #[test]
        fn validate_update_entry_rejects_move_to_new_day_when_target_day_sequence_invalid() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 5, 0).unwrap(),
                    WorkCommand::Log { title: "a".into(), description: None },
                ),
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 10, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    5,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();

            let patch = WorkRecordInput::new(
                Local.with_ymd_and_hms(2025, 1, 11, 9, 6, 0).unwrap(),
                WorkCommand::Log { title: "moved".into(), description: None },
            );

            let err = by_day
                .validate_update_record(&entries, day1[1].id(), &patch)
                .unwrap_err()
                .to_string();
            assert!(err.contains("cannot log while idle"));
        }

        #[test]
        fn validate_rejects_invalid_days() {
            let entry1 = entry(
                3,
                Local.with_ymd_and_hms(2025, 1, 10, 9, 10, 0).unwrap(),
                WorkCommand::End { title: "".into(), description: None },
            );

            let entries = vec![entry1.clone()];
            let entries = id_refs(&entries);
            let err = WorkRecordValidationIndex::try_new(&entries).unwrap_err().to_string();

            assert!(err.contains("cannot end while idle"));
        }

        #[test]
        fn validate_remove_entry_rejects_missing_id() {
            let day = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    WorkCommand::Start { context: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 5, 0).unwrap(),
                    WorkCommand::End { title: "".into(), description: None },
                ),
            ];

            let entries = id_refs(&day);
            let by_day = WorkRecordValidationIndex::try_new(&entries).unwrap();
            let missing_id: RecordId = 999.into();

            let err = by_day.validate_remove_record(&entries, &missing_id).unwrap_err().to_string();

            assert!(err.contains("record 999 should exist during validation"));
        }
    }
}
