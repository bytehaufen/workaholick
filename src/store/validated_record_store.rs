use chrono::NaiveDate;

use crate::domain::{
    AbsencePortion, AbsenceRecord, AbsenceRecordInput, Record, RecordId, RecordInput, WorkRecord,
    WorkRecordValidationIndex,
};

use super::{RecordReader, RecordWriter};

#[derive(Debug)]
pub struct ValidatedRecordStore<STORE> {
    store: STORE,
    records: Vec<Record>,
    work_index: WorkRecordValidationIndex,
}

impl<STORE: RecordReader + RecordWriter> ValidatedRecordStore<STORE> {
    pub fn new(store: STORE) -> anyhow::Result<Self> {
        let records: Vec<Record> = store.iter().cloned().collect();

        let work_records: Vec<_> = store.iter().filter_map(Record::as_work).collect();

        let work_index = Self::build_work_index(&work_records)?;

        Ok(Self { store, records, work_index })
    }

    fn work_records(&self) -> Vec<&WorkRecord> {
        self.store.iter().filter_map(Record::as_work).collect()
    }

    fn build_work_index(records: &[&WorkRecord]) -> anyhow::Result<WorkRecordValidationIndex> {
        WorkRecordValidationIndex::try_new(records)
    }

    fn refresh_index(&mut self) -> anyhow::Result<()> {
        self.work_index = Self::build_work_index(&self.work_records())?;
        Ok(())
    }

    fn sort_records(&mut self) {
        self.records.sort_by_key(Record::sort_key);
    }

    fn absences_on(&self, date: NaiveDate) -> impl Iterator<Item = &AbsenceRecord> {
        self.records.iter().filter_map(move |record| match record {
            Record::Absence(absence_record) if *absence_record.date() == date => {
                Some(absence_record)
            }
            _ => None,
        })
    }

    fn work_records_on(&self, date: NaiveDate) -> impl Iterator<Item = &WorkRecord> {
        self.records.iter().filter_map(move |record| match record {
            Record::Work(work_record) if work_record.date() == date => Some(work_record),
            _ => None,
        })
    }

    fn validate_insert_absence(&self, input: &AbsenceRecordInput) -> anyhow::Result<()> {
        let (full_days, half_days) = self
            .records
            .iter()
            .filter_map(|record| match record {
                Record::Absence(absence_record) if input.date() == absence_record.date() => {
                    Some(absence_record.portion())
                }
                _ => None,
            })
            .fold((0, 0), |(full_days, half_days), portion| match portion {
                AbsencePortion::FullDay => (full_days + 1, half_days),
                AbsencePortion::HalfDay => (full_days, half_days + 1),
            });

        let date = input.date();

        match input.portion() {
            AbsencePortion::FullDay if full_days > 0 || half_days > 0 => {
                anyhow::bail!("absence already exists for {date}")
            }
            AbsencePortion::HalfDay if full_days > 0 => {
                anyhow::bail!("full-day absence already exists for {date}")
            }
            AbsencePortion::HalfDay if half_days >= 2 => {
                anyhow::bail!("two half-day absences already exist for {date}")
            }

            _ => Ok(()),
        }
    }
}

impl<STORE: RecordReader + RecordWriter> RecordReader for ValidatedRecordStore<STORE> {
    fn iter(&self) -> Box<dyn Iterator<Item = &Record> + '_> {
        Box::new(self.records.iter())
    }

    fn get(&self, id: &RecordId) -> Option<&Record> {
        self.store.get(id)
    }

    fn len(&self) -> usize {
        self.records.len()
    }

    fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl<STORE: RecordReader + RecordWriter> RecordWriter for ValidatedRecordStore<STORE> {
    fn next_id(&self) -> RecordId {
        self.store.next_id()
    }

    fn insert(&mut self, input: RecordInput) -> anyhow::Result<RecordId> {
        let fake_id = self.store.next_id();
        match &input {
            RecordInput::Work(work_record_input) => self.work_index.validate_insert_record(
                &self.work_records(),
                work_record_input,
                fake_id,
            )?,
            RecordInput::Absence(absence_record_input) => {
                self.validate_insert_absence(absence_record_input)?
            }
        }

        let id = self.store.insert(input)?;
        let record = self
            .store
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("record {id} missing after insert"))?
            .clone();
        self.records.push(record);
        self.sort_records();
        self.refresh_index()?;

        Ok(id)
    }

    fn delete(&mut self, id: &RecordId) -> anyhow::Result<Record> {
        self.work_index.validate_remove_record(&self.work_records(), id)?;
        // FIXME: Add absence validation

        let record = self.store.delete(id)?;
        let idx = self
            .records
            .iter()
            .position(|existing| existing.id() == id)
            .ok_or_else(|| anyhow::anyhow!("record {id} missing in validation cache"))?;
        self.records.remove(idx);
        self.refresh_index()?;

        Ok(record)
    }

    fn update(&mut self, id: &RecordId, input: RecordInput) -> anyhow::Result<()> {
        let existing = self
            .records
            .iter()
            .find(|record| record.id() == id)
            .ok_or_else(|| anyhow::anyhow!("record {id} missing in validation cache"))?;

        match (existing, &input) {
            (Record::Work(..), RecordInput::Work(work_input)) => {
                self.work_index.validate_update_record(&self.work_records(), id, work_input)?
            }
            // FIXME: Implement
            (Record::Absence(..), RecordInput::Absence(absence_input)) => todo!(),

            // Changing record type is not allowed
            (Record::Work(..), RecordInput::Absence(..)) => anyhow::bail!(
                "record {id} is a work record and cannot be updated with absence input"
            ),
            (Record::Absence(..), RecordInput::Work(..)) => anyhow::bail!(
                "record {id} is an absence record and cannot be updated with work input"
            ),
        }

        self.store.update(id, input.clone())?;
        let idx = self
            .records
            .iter()
            .position(|existing| existing.id() == id)
            .ok_or_else(|| anyhow::anyhow!("record {id} missing in validation cache"))?;
        self.records[idx] = Record::from_input(*id, input);
        self.sort_records();
        self.refresh_index()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Context, Result};
    use chrono::{Local, NaiveDate, TimeZone};
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::domain::{
        AbsenceKind, AbsencePortion, AbsenceRecordInput, RecordInput, WorkCommand, WorkContext,
        WorkRecordInput,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum StoreCall {
        Iter,
        Insert,
        Remove,
        Update,
        Get,
    }

    #[derive(Debug, Default)]
    struct FakeEntryStore {
        entries: Vec<(RecordId, Record)>,
        next_id: u64,
        calls: Rc<RefCell<Vec<StoreCall>>>,
    }

    impl FakeEntryStore {
        fn seed(mut self, inputs: Vec<(RecordId, RecordInput)>) -> Self {
            inputs.into_iter().for_each(|(id, input)| {
                self.entries.push((id, Record::from_input(id, input)));
                self.next_id = self.next_id.max(id.value() + 1);
            });

            self.sort_records();
            self
        }

        fn calls_handle(&self) -> Rc<RefCell<Vec<StoreCall>>> {
            Rc::clone(&self.calls)
        }

        fn sort_records(&mut self) {
            self.entries.sort_by_key(|(_, record)| record.sort_key());
        }
    }

    impl RecordReader for FakeEntryStore {
        fn iter(&self) -> Box<dyn Iterator<Item = &Record> + '_> {
            self.calls.borrow_mut().push(StoreCall::Iter);
            Box::new(self.entries.iter().map(|(_, entry)| entry))
        }

        fn get(&self, id: &RecordId) -> Option<&Record> {
            self.calls.borrow_mut().push(StoreCall::Get);
            self.entries.iter().find(|(entry_id, _)| entry_id == id).map(|(_, entry)| entry)
        }

        fn len(&self) -> usize {
            self.entries.len()
        }

        fn is_empty(&self) -> bool {
            self.entries.is_empty()
        }
    }

    impl RecordWriter for FakeEntryStore {
        fn next_id(&self) -> RecordId {
            RecordId::from(self.next_id)
        }

        fn insert(&mut self, input: RecordInput) -> Result<RecordId> {
            self.calls.borrow_mut().push(StoreCall::Insert);

            let id = RecordId::from(self.next_id);
            self.next_id += 1;
            self.entries.push((id, Record::from_input(id, input)));
            self.sort_records();

            Ok(id)
        }

        fn delete(&mut self, id: &RecordId) -> Result<Record> {
            self.calls.borrow_mut().push(StoreCall::Remove);
            let idx = self
                .entries
                .iter()
                .position(|(entry_id, _)| entry_id == id)
                .with_context(|| format!("id {id} does not exist"))?;
            Ok(self.entries.remove(idx).1)
        }

        fn update(&mut self, id: &RecordId, input: RecordInput) -> Result<()> {
            self.calls.borrow_mut().push(StoreCall::Update);
            let idx = self
                .entries
                .iter()
                .position(|(entry_id, _)| entry_id == id)
                .with_context(|| format!("id {id} does not exist"))?;
            self.entries[idx].1 = Record::from_input(*id, input);
            self.sort_records();
            Ok(())
        }
    }

    fn work_input(timestamp: chrono::DateTime<Local>, command: WorkCommand) -> RecordInput {
        RecordInput::Work(WorkRecordInput::new(timestamp, command))
    }

    #[test]
    fn insert_allows_cross_day_log_when_previous_day_is_active() {
        let day_1 = Local.with_ymd_and_hms(2026, 1, 10, 9, 0, 0).unwrap();
        let day_2 = Local.with_ymd_and_hms(2026, 1, 11, 10, 0, 0).unwrap();

        let store = FakeEntryStore::default().seed(vec![(
            42.into(),
            work_input(day_1, WorkCommand::Start { context: WorkContext::Office }),
        )]);
        let calls = store.calls_handle();

        let mut store = ValidatedRecordStore::new(store).unwrap();

        let result = store.insert(work_input(
            day_2,
            WorkCommand::Log { title: "standup".into(), description: None },
        ));

        assert!(result.is_ok());

        let calls = calls.borrow();
        assert!(calls.contains(&StoreCall::Iter));
        assert!(calls.contains(&StoreCall::Insert));
    }

    #[test]
    fn insert_reports_next_id_in_validation_error() {
        let day_1_start = Local.with_ymd_and_hms(2026, 1, 10, 9, 0, 0).unwrap();
        let day_1_end = Local.with_ymd_and_hms(2026, 1, 10, 10, 0, 0).unwrap();
        let day_2 = Local.with_ymd_and_hms(2026, 1, 11, 10, 0, 0).unwrap();

        let store = FakeEntryStore::default().seed(vec![
            (
                1.into(),
                work_input(day_1_start, WorkCommand::Start { context: WorkContext::Office }),
            ),
            (
                2.into(),
                work_input(day_1_end, WorkCommand::End { title: "".into(), description: None }),
            ),
        ]);

        let mut store = ValidatedRecordStore::new(store).unwrap();

        let err = store
            .insert(work_input(
                day_2,
                WorkCommand::Log { title: "standup".into(), description: None },
            ))
            .unwrap_err()
            .to_string();

        assert_eq!(err, "Day entries validation failed: Entry 3 - cannot log while idle");
    }

    #[test]
    fn remove_when_validation_would_break_is_rejected_before_removal() {
        let ts1 = Local.with_ymd_and_hms(2026, 1, 10, 9, 0, 0).unwrap();
        let ts2 = Local.with_ymd_and_hms(2026, 1, 10, 10, 0, 0).unwrap();

        let store = FakeEntryStore::default().seed(vec![
            (42.into(), work_input(ts1, WorkCommand::Start { context: WorkContext::Office })),
            (43.into(), work_input(ts2, WorkCommand::Log { title: "".into(), description: None })),
        ]);
        let calls = store.calls_handle();

        let mut store = ValidatedRecordStore::new(store).unwrap();

        let result = store.delete(&42.into());
        result.expect_err("expected validation error before write");

        let calls = calls.borrow();
        assert!(calls.contains(&StoreCall::Iter));
        assert!(!calls.contains(&StoreCall::Remove));
    }

    #[test]
    fn update_when_validation_would_break_is_rejected_before_update() {
        let ts1 = Local.with_ymd_and_hms(2026, 1, 10, 9, 0, 0).unwrap();
        let ts2 = Local.with_ymd_and_hms(2026, 1, 10, 10, 0, 0).unwrap();

        let store = FakeEntryStore::default().seed(vec![
            (42.into(), work_input(ts1, WorkCommand::Start { context: WorkContext::Office })),
            (43.into(), work_input(ts2, WorkCommand::Log { title: "".into(), description: None })),
        ]);
        let calls = store.calls_handle();

        let patch = work_input(ts1, WorkCommand::End { title: "".into(), description: None });

        let mut store = ValidatedRecordStore::new(store).unwrap();

        let result = store.update(&42.into(), patch);
        result.expect_err("expected validation error before write");

        let calls = calls.borrow();
        assert!(calls.contains(&StoreCall::Iter));
        assert!(!calls.contains(&StoreCall::Update));
    }

    #[test]
    fn len_and_is_empty_reflect_store_state() {
        let mut store = ValidatedRecordStore::new(FakeEntryStore::default()).unwrap();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        store
            .insert(work_input(
                Local.with_ymd_and_hms(2026, 1, 10, 9, 0, 0).unwrap(),
                WorkCommand::Start { context: WorkContext::Office },
            ))
            .unwrap();

        assert!(!store.is_empty());
        assert_eq!(store.len(), 1);

        store.delete(&RecordId::from(0)).unwrap();

        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn update_rejects_changing_record_type() {
        let ts = Local.with_ymd_and_hms(2026, 5, 3, 5, 30, 0).unwrap();
        let store = FakeEntryStore::default().seed(vec![(
            69.into(),
            work_input(ts, WorkCommand::Start { context: WorkContext::Home }),
        )]);

        let calls = store.calls_handle();

        let input = RecordInput::Absence(AbsenceRecordInput::new(
            NaiveDate::from_ymd_opt(2026, 5, 3).unwrap(),
            AbsenceKind::Vacation,
            AbsencePortion::FullDay,
            Some("".to_string()),
        ));

        let mut store = ValidatedRecordStore::new(store).unwrap();

        let result = store.update(&69.into(), input);

        result.expect_err("record 69 is a work record and cannot be updated with absence input");

        let calls = calls.borrow();
        assert!(!calls.contains(&StoreCall::Update))
    }
}
