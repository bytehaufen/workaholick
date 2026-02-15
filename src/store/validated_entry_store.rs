use crate::{
    EntryStore,
    domain::{Entry, EntryId, EntryMutation, EntryPatch, EntryValidationIndex},
};

#[derive(Debug)]
pub struct ValidatedEntryStore<STORE> {
    store: STORE,
}

impl<STORE: EntryStore> ValidatedEntryStore<STORE> {
    pub fn new(store: STORE) -> anyhow::Result<Self> {
        let entries: Vec<(&EntryId, &Entry)> = store.iter().collect();
        EntryValidationIndex::validate_snapshot(&entries)?;

        Ok(Self { store })
    }

    fn entries(&self) -> Vec<(&EntryId, &Entry)> {
        self.store.iter().collect()
    }
}

impl<STORE: EntryStore> EntryStore for ValidatedEntryStore<STORE> {
    fn insert(&mut self, entry: EntryPatch) -> anyhow::Result<EntryId> {
        let entries = self.entries();
        EntryValidationIndex::validate(&entries, EntryMutation::Insert { patch: &entry })?;

        let id = self.store.insert(entry)?;

        Ok(id)
    }

    fn iter(&self) -> impl Iterator<Item = (&EntryId, &Entry)> {
        self.store.iter()
    }

    fn remove(&mut self, id: &EntryId) -> anyhow::Result<Entry> {
        let entries = self.entries();
        EntryValidationIndex::validate(&entries, EntryMutation::Remove { id })?;

        let entry = self.store.remove(id)?;

        Ok(entry)
    }

    fn update(&mut self, id: &EntryId, patch: EntryPatch) -> anyhow::Result<()> {
        let entries = self.entries();
        EntryValidationIndex::validate(&entries, EntryMutation::Update { id, patch: &patch })?;

        self.store.update(id, patch)?;

        Ok(())
    }

    fn get(&self, id: &EntryId) -> Option<&Entry> {
        self.store.get(id)
    }
}
