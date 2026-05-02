use std::{collections::BTreeMap, ops::Bound};

use chrono::NaiveDate;

use crate::domain::{RecordId, WorkRecord, WorkRecordInput, day_entries::DayState};

use super::validation::{DayRecordInfo, DayRecordValidationError};

#[derive(Debug, PartialEq)]
pub(crate) struct WorkRecordValidationIndex {
    pub(super) days: BTreeMap<NaiveDate, DayRecordInfo>,
}

impl WorkRecordValidationIndex {
    pub(crate) fn validate_insert_record(
        &self,
        records: &[&WorkRecord],
        input: &WorkRecordInput,
        new_record_id: RecordId,
    ) -> anyhow::Result<()> {
        let date = input.date();
        let dummy = WorkRecord::from_input(new_record_id, input.clone());
        let initial_state = self.state_before(date);

        let mut relevant_records: Vec<&WorkRecord> = Vec::new();

        let resolve = |id| -> anyhow::Result<&WorkRecord> { Self::resolve_record(records, id) };

        // Current day (inclusive new one)
        if let Some(day) = self.days.get(&date) {
            relevant_records
                .extend(day.ids.iter().map(resolve).collect::<anyhow::Result<Vec<&WorkRecord>>>()?);
        }

        relevant_records.push(&dummy);

        // Next day
        if let Some((_, next_day)) =
            self.days.range((Bound::Excluded(date), Bound::Unbounded)).next()
        {
            relevant_records.extend(
                next_day.ids.iter().map(resolve).collect::<anyhow::Result<Vec<&WorkRecord>>>()?,
            );
        }

        Self::try_new_with_initial_state(&relevant_records, initial_state)?;

        Ok(())
    }

    pub(crate) fn validate_remove_record(
        &self,
        records: &[&WorkRecord],
        id: &RecordId,
    ) -> anyhow::Result<()> {
        let record = Self::resolve_record(records, id)?;
        let date = record.date();
        let initial_state = self.state_before(date);

        let mut relevant_records: Vec<&WorkRecord> = Vec::new();

        let resolve = |id| -> anyhow::Result<&WorkRecord> { Self::resolve_record(records, id) };

        // Current day, excluding the removed one
        let day = self.days.get(&date).expect("current day should exist");
        relevant_records.extend(
            day.ids
                .iter()
                .filter(|&record_id| record_id != id)
                .map(resolve)
                .collect::<anyhow::Result<Vec<&WorkRecord>>>()?,
        );

        // Next day
        if let Some((_, next_day)) =
            self.days.range((Bound::Excluded(date), Bound::Unbounded)).next()
        {
            relevant_records.extend(
                next_day.ids.iter().map(resolve).collect::<anyhow::Result<Vec<&WorkRecord>>>()?,
            );
        }

        if !relevant_records.is_empty() {
            Self::try_new_with_initial_state(&relevant_records, initial_state)?;
        }

        Ok(())
    }

    pub(crate) fn validate_update_record(
        &self,
        records: &[&WorkRecord],
        id: &RecordId,
        input: &WorkRecordInput,
    ) -> anyhow::Result<()> {
        let record = Self::resolve_record(records, id)?;
        let old_date = record.date();
        let new_date = input.date();

        if old_date != new_date {
            self.validate_remove_record(records, id)?;

            let mut records_without_target: Vec<&WorkRecord> = records.to_vec();
            records_without_target.retain(|record| record.id() != id);

            let by_day_without_target = Self::try_new(&records_without_target)?;
            by_day_without_target.validate_insert_record(&records_without_target, input, *id)?;
        } else {
            let updated = WorkRecord::from_input(*id, input.clone());
            let initial_state = self.state_before(input.date());
            let resolve = |record_id| -> anyhow::Result<&WorkRecord> {
                Self::resolve_record(records, record_id)
            };

            let mut relevant_records: Vec<&WorkRecord> = Vec::new();

            // Current day
            let day = self.days.get(&input.date()).expect("current day should exist");
            relevant_records.extend(
                day.ids
                    .iter()
                    .filter(|record_id| *record_id != id)
                    .map(resolve)
                    .collect::<anyhow::Result<Vec<&WorkRecord>>>()?,
            );
            relevant_records.push(&updated);

            // Next day
            if let Some((_, next_day)) =
                self.days.range((Bound::Excluded(input.date()), Bound::Unbounded)).next()
            {
                relevant_records.extend(
                    next_day
                        .ids
                        .iter()
                        .map(resolve)
                        .collect::<anyhow::Result<Vec<&WorkRecord>>>()?,
                );
            }

            let _ = Self::try_new_with_initial_state(&relevant_records, initial_state)?;
        }

        Ok(())
    }

    pub(crate) fn try_new(records: &[&WorkRecord]) -> anyhow::Result<Self> {
        let days = index_records_by_date(records)?;

        Ok(Self { days })
    }

    fn try_new_with_initial_state(
        records: &[&WorkRecord],
        initial_state: DayState,
    ) -> anyhow::Result<Self> {
        let days = index_records_by_date_with_initial_state(records, initial_state)?;

        Ok(Self { days })
    }

    fn state_before(&self, date: NaiveDate) -> DayState {
        self.days.range(..date).next_back().map(|(_, day)| day.end_state).unwrap_or_default()
    }

    fn resolve_record<'a>(
        records: &'a [&WorkRecord],
        id: &RecordId,
    ) -> anyhow::Result<&'a WorkRecord> {
        let record = records
            .iter()
            .find(|record| record.id() == id)
            .ok_or_else(|| anyhow::anyhow!("record {id} should exist during validation"))?;

        Ok(*record)
    }
}

pub(super) fn index_records_by_date(
    records: &[&WorkRecord],
) -> Result<BTreeMap<NaiveDate, DayRecordInfo>, DayRecordValidationError> {
    index_records_by_date_with_initial_state(records, DayState::Idle)
}

fn index_records_by_date_with_initial_state(
    records: &[&WorkRecord],
    initial_state: DayState,
) -> Result<BTreeMap<NaiveDate, DayRecordInfo>, DayRecordValidationError> {
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
            day_records.insert(*date, day_record);

            Ok((day_records, next_state))
        },
    )?;

    Ok(day_records)
}
