use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::domain::work_transition::validate_day_records;
use crate::domain::{WorkRecord, work_transition::DayState};

use crate::domain::{DayRecordValidationError, DayRecordValidationIssue};

pub(crate) fn validate_work_timeline(
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
    if records.is_empty() {
        return Err(DayRecordValidationError::new(DayRecordValidationIssue::EmptyRecords));
    }
    let first_date = records[0].date();

    if !records.iter().all(|record| record.date() == first_date) {
        return Err(DayRecordValidationError::new(DayRecordValidationIssue::MixedDates));
    }

    let end_state = validate_day_records(records, &prev_state)?;

    Ok(end_state)
}

#[cfg(test)]
mod tests {
    use super::super::{WorkCommand, WorkRecord};
    use crate::test_support::*;

    use chrono::{Local, TimeZone};

    fn record(id: u64, ts: chrono::DateTime<Local>, cmd: WorkCommand) -> WorkRecord {
        WorkRecord::new(id.into(), ts, cmd)
    }

    use crate::domain::work_timeline_validation::validate_work_day;

    #[test]
    fn try_from_empty_returns_error() {
        let err = validate_work_day(&Vec::new(), Default::default()).unwrap_err();
        assert_eq!(err.to_string(), "Day entries validation failed: entries must not be empty");
    }

    #[test]
    fn try_from_mixed_dates_returns_error() {
        let record1 = record(1, ts(4, 16, 8), WorkCommand::default());
        let record2 = record(
            2,
            Local.with_ymd_and_hms(4025, 1, 4, 16, 9, 0).unwrap(),
            WorkCommand::default(),
        );

        let err = validate_work_day(&[&record1, &record2], Default::default()).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed: entries must all be of the same date"
        );
    }
}
