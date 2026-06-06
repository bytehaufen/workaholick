use crate::domain::{Record, RecordChange, RecordId, RecordInput, RecordSet, validate_records};

use super::{RecordReader, RecordWriter};

#[derive(Debug)]
pub struct ValidatedRecordStore<STORE> {
    store: STORE,
}

impl<STORE: RecordReader + RecordWriter> ValidatedRecordStore<STORE> {
    pub fn new(store: STORE) -> anyhow::Result<Self> {
        let records: Vec<Record> = store.iter().cloned().collect();
        let record_set = RecordSet::try_from_records(records)?;
        validate_records(&record_set)?;

        Ok(Self { store })
    }

    fn validate_change(&self, change: &RecordChange) -> anyhow::Result<()> {
        let records: Vec<_> = self.store.iter().cloned().collect();
        let mut candidate = RecordSet::try_from_records(records)?;
        let _change_outcome = candidate.apply_change(change)?;

        validate_records(&candidate)
    }
}

impl<STORE: RecordReader + RecordWriter> RecordReader for ValidatedRecordStore<STORE> {
    fn iter(&self) -> Box<dyn Iterator<Item = &Record> + '_> {
        self.store.iter()
    }

    fn get(&self, id: &RecordId) -> Option<&Record> {
        self.store.get(id)
    }

    fn len(&self) -> usize {
        self.store.len()
    }

    fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

impl<STORE: RecordReader + RecordWriter> RecordWriter for ValidatedRecordStore<STORE> {
    fn next_id(&self) -> RecordId {
        self.store.next_id()
    }

    fn insert(&mut self, input: RecordInput) -> anyhow::Result<RecordId> {
        let record_change = RecordChange::Insert { id: self.store.next_id(), input: input.clone() };

        self.validate_change(&record_change)?;
        self.store.insert(input)
    }

    fn delete(&mut self, id: &RecordId) -> anyhow::Result<Record> {
        let record_change = RecordChange::Delete { id: *id };

        self.validate_change(&record_change)?;
        self.store.delete(id)
    }

    fn update(&mut self, id: &RecordId, input: RecordInput) -> anyhow::Result<()> {
        let record_change = RecordChange::Update { id: *id, input: input.clone() };

        self.validate_change(&record_change)?;
        self.store.update(id, input)
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
