use crate::domain::{Entry, EntryId, EntryPatch};

use anyhow::Result;

pub trait EntryReader {
    /// This is sorted by timestamp
    fn iter(&self) -> Box<dyn Iterator<Item = &Entry> + '_>;
    fn get(&self, id: &EntryId) -> Option<&Entry>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

pub trait EntryWriter {
    fn next_id(&self) -> EntryId;
    fn insert(&mut self, entry: EntryPatch) -> Result<EntryId>;
    fn remove(&mut self, id: &EntryId) -> Result<Entry>;
    fn update(&mut self, id: &EntryId, patch: EntryPatch) -> Result<()>;
}

pub trait EntryStore: EntryReader + EntryWriter {}
impl<T: EntryReader + EntryWriter> EntryStore for T {}
