use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::domain::{WorkRecord, work_transition::DayState};

use super::validation::{DayRecordInfo, DayRecordValidationError};

pub(crate) fn validate_work_timeline(
    records: &[&WorkRecord],
) -> Result<BTreeMap<NaiveDate, DayRecordInfo>, DayRecordValidationError> {
    let initial_state = DayState::Idle;

    let mut records_by_date: BTreeMap<NaiveDate, Vec<&WorkRecord>> =
        records.iter().fold(BTreeMap::new(), |mut acc, record| {
            acc.entry(record.date()).or_default().push(record);
            acc
        });

    records_by_date
        .values_mut()
        .for_each(|records| records.sort_by_key(|record| record.sort_key()));

    let (day_records, _) = records_by_date.iter().try_fold(
        (BTreeMap::new(), initial_state),
        |(mut day_records, last_state), (date, records)| {
            let day_record = DayRecordInfo::try_new(records, &last_state)?;

            let next_state = day_record.end_state;
            let _ = day_records.insert(*date, day_record);

            Ok((day_records, next_state))
        },
    )?;

    Ok(day_records)
}
