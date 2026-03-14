use crate::domain::{Entry, EntryId, EntryPatch, EntryValidationIndex};

use super::{EntryReader, EntryWriter};

#[derive(Debug)]
pub struct ValidatedEntryStore<STORE> {
    store: STORE,
    entries: Vec<Entry>,
    index: EntryValidationIndex,
}

impl<STORE: EntryReader + EntryWriter> ValidatedEntryStore<STORE> {
    pub fn new(store: STORE) -> anyhow::Result<Self> {
        let entries: Vec<Entry> = store.iter().cloned().collect();
        let index = Self::build_index(&entries)?;

        Ok(Self { store, entries, index })
    }

    fn entries(&self) -> Vec<&Entry> {
        self.entries.iter().collect()
    }

    fn build_index(entries: &[Entry]) -> anyhow::Result<EntryValidationIndex> {
        let refs: Vec<&Entry> = entries.iter().collect();
        EntryValidationIndex::try_new(&refs)
    }

    fn refresh_index(&mut self) -> anyhow::Result<()> {
        self.index = Self::build_index(&self.entries)?;
        Ok(())
    }

    fn sort_entries(&mut self) {
        self.entries.sort_by_key(|entry| entry.sort_key());
    }
}

impl<STORE: EntryReader + EntryWriter> EntryReader for ValidatedEntryStore<STORE> {
    fn iter(&self) -> Box<dyn Iterator<Item = &Entry> + '_> {
        Box::new(self.entries.iter())
    }

    fn get(&self, id: &EntryId) -> Option<&Entry> {
        self.store.get(id)
    }
}

impl<STORE: EntryReader + EntryWriter> EntryWriter for ValidatedEntryStore<STORE> {
    fn next_id(&self) -> EntryId {
        self.store.next_id()
    }

    fn insert(&mut self, entry: EntryPatch) -> anyhow::Result<EntryId> {
        let entries = self.entries();
        let new_id = self.store.next_id();
        self.index.validate_insert_entry(&entries, &entry, new_id)?;

        let id = self.store.insert(entry)?;
        let entry = self
            .store
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("entry {id} missing after insert"))?
            .clone();
        self.entries.push(entry);
        self.sort_entries();
        self.refresh_index()?;

        Ok(id)
    }

    fn remove(&mut self, id: &EntryId) -> anyhow::Result<Entry> {
        let entries = self.entries();
        self.index.validate_remove_entry(&entries, id)?;

        let entry = self.store.remove(id)?;
        let idx = self
            .entries
            .iter()
            .position(|existing| existing.id() == id)
            .ok_or_else(|| anyhow::anyhow!("entry {id} missing in validation cache"))?;
        self.entries.remove(idx);
        self.refresh_index()?;

        Ok(entry)
    }

    fn update(&mut self, id: &EntryId, patch: EntryPatch) -> anyhow::Result<()> {
        let entries = self.entries();
        self.index.validate_update_entry(&entries, id, &patch)?;

        self.store.update(id, patch.clone())?;
        let idx = self
            .entries
            .iter()
            .position(|existing| existing.id() == id)
            .ok_or_else(|| anyhow::anyhow!("entry {id} missing in validation cache"))?;
        let version = *self.entries[idx].version();
        self.entries[idx] = Entry::from_patch(*id, version, patch);
        self.sort_entries();
        self.refresh_index()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use anyhow::{Context, Result};
    use chrono::{Local, TimeZone};
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::domain::{ActivityKind, Command, WorkContext};

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
        entries: Vec<(EntryId, Entry)>,
        next_id: u64,
        calls: Rc<RefCell<Vec<StoreCall>>>,
    }

    impl FakeEntryStore {
        fn seed(mut self, patches: Vec<(EntryId, EntryPatch)>) -> Self {
            patches.into_iter().for_each(|(id, patch)| {
                self.entries.push((id, Entry::from_patch(id, 1.into(), patch)));
                self.next_id = self.next_id.max(id.value() + 1);
            });

            self.sort_by_timestamp();
            self
        }

        fn calls_handle(&self) -> Rc<RefCell<Vec<StoreCall>>> {
            Rc::clone(&self.calls)
        }

        fn sort_by_timestamp(&mut self) {
            self.entries.sort_by(|a, b| a.1.timestamp().cmp(b.1.timestamp()));
        }
    }

    impl EntryReader for FakeEntryStore {
        fn iter(&self) -> Box<dyn Iterator<Item = &Entry> + '_> {
            self.calls.borrow_mut().push(StoreCall::Iter);
            Box::new(self.entries.iter().map(|(_, entry)| entry))
        }

        fn get(&self, id: &EntryId) -> Option<&Entry> {
            self.calls.borrow_mut().push(StoreCall::Get);
            self.entries.iter().find(|(entry_id, _)| entry_id == id).map(|(_, entry)| entry)
        }
    }

    impl EntryWriter for FakeEntryStore {
        fn next_id(&self) -> EntryId {
            EntryId::from(self.next_id)
        }

        fn insert(&mut self, patch: EntryPatch) -> Result<EntryId> {
            self.calls.borrow_mut().push(StoreCall::Insert);

            let id = EntryId::from(self.next_id);
            self.next_id += 1;
            self.entries.push((id, Entry::from_patch(id, 1.into(), patch)));
            self.sort_by_timestamp();

            Ok(id)
        }

        fn remove(&mut self, id: &EntryId) -> Result<Entry> {
            self.calls.borrow_mut().push(StoreCall::Remove);
            let idx = self
                .entries
                .iter()
                .position(|(entry_id, _)| entry_id == id)
                .with_context(|| format!("id {id} does not exist"))?;
            Ok(self.entries.remove(idx).1)
        }

        fn update(&mut self, id: &EntryId, patch: EntryPatch) -> Result<()> {
            self.calls.borrow_mut().push(StoreCall::Update);
            let idx = self
                .entries
                .iter()
                .position(|(entry_id, _)| entry_id == id)
                .with_context(|| format!("id {id} does not exist"))?;
            let version = *self.entries[idx].1.version();
            self.entries[idx].1 = Entry::from_patch(*id, version, patch);
            self.sort_by_timestamp();
            Ok(())
        }
    }

    #[test]
    fn insert_allows_cross_day_log_when_previous_day_is_active() {
        let day_1 = Local.with_ymd_and_hms(2026, 1, 10, 9, 0, 0).unwrap();
        let day_2 = Local.with_ymd_and_hms(2026, 1, 11, 10, 0, 0).unwrap();

        let store = FakeEntryStore::default().seed(vec![(
            42.into(),
            EntryPatch::new(
                day_1,
                Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } },
            ),
        )]);
        let calls = store.calls_handle();

        let mut store = ValidatedEntryStore::new(store).unwrap();

        let result = store.insert(EntryPatch::new(
            day_2,
            Command::Log { title: "standup".into(), description: None },
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
                EntryPatch::new(
                    day_1_start,
                    Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } },
                ),
            ),
            (
                2.into(),
                EntryPatch::new(day_1_end, Command::End { title: "".into(), description: None }),
            ),
        ]);

        let mut store = ValidatedEntryStore::new(store).unwrap();

        let err = store
            .insert(EntryPatch::new(
                day_2,
                Command::Log { title: "standup".into(), description: None },
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
            (
                42.into(),
                EntryPatch::new(
                    ts1,
                    Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } },
                ),
            ),
            (43.into(), EntryPatch::new(ts2, Command::Log { title: "".into(), description: None })),
        ]);
        let calls = store.calls_handle();

        let mut store = ValidatedEntryStore::new(store).unwrap();

        let result = store.remove(&42.into());
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
            (
                42.into(),
                EntryPatch::new(
                    ts1,
                    Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } },
                ),
            ),
            (43.into(), EntryPatch::new(ts2, Command::Log { title: "".into(), description: None })),
        ]);
        let calls = store.calls_handle();

        let patch = EntryPatch::new(ts1, Command::End { title: "".into(), description: None });

        let mut store = ValidatedEntryStore::new(store).unwrap();

        let result = store.update(&42.into(), patch);
        result.expect_err("expected validation error before write");

        let calls = calls.borrow();
        assert!(calls.contains(&StoreCall::Iter));
        assert!(!calls.contains(&StoreCall::Update));
    }
}
