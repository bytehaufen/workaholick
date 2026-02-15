use crate::{
    EntryStore,
    domain::{EntriesByDay, Entry, EntryId, EntryPatch},
};

#[derive(Debug)]
pub struct ValidatedEntryStore<STORE> {
    store: STORE,
    entries_by_day: EntriesByDay,
}

impl<STORE: EntryStore> ValidatedEntryStore<STORE> {
    pub fn new(store: STORE) -> anyhow::Result<Self> {
        let entries: Vec<(&EntryId, &Entry)> = store.iter().collect();

        let entries_by_day = EntriesByDay::try_new(&entries)?;

        Ok(Self { store, entries_by_day })
    }
}

impl<STORE: EntryStore> EntryStore for ValidatedEntryStore<STORE> {
    fn insert(&mut self, entry: EntryPatch) -> anyhow::Result<EntryId> {
        let entries: Vec<(&EntryId, &Entry)> = self.store.iter().collect();
        let validated_entry = self.entries_by_day.validate_insert_entry(&entries, &entry)?;

        let id = self.store.insert(entry)?;
        self.entries_by_day.apply_insert(id, validated_entry);

        Ok(id)
    }

    fn iter(&self) -> impl Iterator<Item = (&EntryId, &Entry)> {
        self.store.iter()
    }

    fn remove(&mut self, id: &EntryId) -> anyhow::Result<Entry> {
        let entries: Vec<(&EntryId, &Entry)> = self.store.iter().collect();
        let validated_entry = self.entries_by_day.validate_remove_entry(&entries, id)?;

        let entry = self.store.remove(id)?;

        self.entries_by_day.apply_remove(validated_entry);

        Ok(entry)
    }

    fn update(&mut self, id: &EntryId, patch: EntryPatch) -> anyhow::Result<()> {
        let entries: Vec<(&EntryId, &Entry)> = self.store.iter().collect();
        let validated_update = self.entries_by_day.validate_update_entry(&entries, id, &patch)?;

        self.store.update(id, patch)?;

        self.entries_by_day.apply_update(validated_update);
        Ok(())
    }

    fn get(&self, id: &EntryId) -> Option<&Entry> {
        self.store.get(id)
    }
}
