use crate::domain::{Entry, EntryId, NewEntry};

pub trait EntryStore {
    fn append(&mut self, entry: NewEntry) -> anyhow::Result<()>;
    fn entries(&self) -> &[Entry];
    fn delete_by_id(&mut self, id: &EntryId) -> anyhow::Result<bool>;
    fn update(&mut self, id: EntryId, patch: NewEntry) -> anyhow::Result<()>;
    fn find_by_id(&self, id: EntryId) -> Option<&Entry>;
}
