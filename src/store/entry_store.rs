use crate::domain::{Entry, EntryId, EntryPatch};

use anyhow::Result;

pub trait EntryStore {
    fn insert(&mut self, entry: EntryPatch) -> Result<EntryId>;
    /// This is sorted by timestamp
    fn iter(&self) -> impl Iterator<Item = (&EntryId, &Entry)>;
    fn remove(&mut self, id: &EntryId) -> Result<Entry>;
    fn update(&mut self, id: &EntryId, patch: EntryPatch) -> Result<()>;
    fn get(&self, id: &EntryId) -> Option<&Entry>;
}
