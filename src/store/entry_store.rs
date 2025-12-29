use crate::domain::{Entry, EntryId, NewEntry};

use anyhow::Result;

pub trait EntryStore {
    fn append(&mut self, entry: NewEntry) -> Result<()>;
    fn entries(&self) -> &[Entry];
    fn delete_by_id(&mut self, id: &EntryId) -> Result<bool>;
    fn update(&mut self, id: EntryId, patch: NewEntry) -> Result<()>;
    fn find_by_id(&self, id: EntryId) -> Option<&Entry>;
}
