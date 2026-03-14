use crate::domain::{Entry, EntryId, EntryPatch};

use anyhow::Result;

pub trait EntryReader {
    fn next_id(&self) -> EntryId;
    /// This is sorted by timestamp
    fn iter(&self) -> impl Iterator<Item = &Entry>;
    fn get(&self, id: &EntryId) -> Option<&Entry>;
}

pub trait EntryWriter {
    fn insert(&mut self, entry: EntryPatch) -> Result<EntryId>;
    fn remove(&mut self, id: &EntryId) -> Result<Entry>;
    fn update(&mut self, id: &EntryId, patch: EntryPatch) -> Result<()>;
}

pub trait EntryStore: EntryReader + EntryWriter {}
impl<T: EntryReader + EntryWriter> EntryStore for T {}
