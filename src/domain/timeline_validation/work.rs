use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::domain::{WorkRecord, work_state::DayState};

use crate::domain::{DayRecordValidationError, transition_day_state};

pub(crate) fn validate_work_records(
    records: &[&WorkRecord],
) -> Result<(), DayRecordValidationError> {
    let initial_state = DayState::Idle;

    let mut records_by_date = group_records_by_date(records);

    records_by_date
        .values_mut()
        .for_each(|records| records.sort_by_key(|record| record.sort_key()));

    let _ = records_by_date
        .values()
        .try_fold(initial_state, |prev_state, records| validate_work_day(records, prev_state))?;

    Ok(())
}

fn group_records_by_date<'a>(
    records: &[&'a WorkRecord],
) -> BTreeMap<NaiveDate, Vec<&'a WorkRecord>> {
    records.iter().fold(BTreeMap::new(), |mut acc, record| {
        acc.entry(record.date()).or_default().push(record);
        acc
    })
}

fn validate_work_day(
    records: &[&WorkRecord],
    prev_state: DayState,
) -> Result<DayState, DayRecordValidationError> {
    records.iter().try_fold(prev_state, |state, record| {
        transition_day_state(state, *record.id(), record.command())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{WorkCommand, WorkContext, WorkRecord, work_state::DayState};
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

        assert!(validate_work_day(&records, Default::default()).is_ok());
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

        assert!(validate_work_day(&records, Default::default()).is_ok());
    }

    #[test]
    fn given_active_start_returns_error() {
        let cmds = [
            WorkCommand::Start { context: Default::default() },
            WorkCommand::Start { context: Default::default() },
        ];
        let records = records_at_same_day(&cmds);
        let records = refs(&records);

        let err = validate_work_day(&records, Default::default()).unwrap_err();
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

        let err = validate_work_day(&records, Default::default()).unwrap_err();
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

        let err = validate_work_day(&records, Default::default()).unwrap_err();
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

        let err = validate_work_day(&records, Default::default()).unwrap_err();
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

        let err = validate_work_day(&records, Default::default()).unwrap_err();
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

        let err = validate_work_day(&records, Default::default()).unwrap_err();
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

        let err = validate_work_day(&records, Default::default()).unwrap_err();
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

        let err = validate_work_day(&records, Default::default()).unwrap_err();
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

        let end_state = validate_work_day(&records, prev_state).unwrap();
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

        let end_state = validate_work_day(&records, prev_state).unwrap();
        assert_eq!(end_state, DayState::Idle);
    }
}
