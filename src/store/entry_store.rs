use crate::domain::Entry;

pub trait EntryStore {
    fn append(&mut self, entry: &Entry) -> anyhow::Result<()>;
    fn read_all(&self) -> anyhow::Result<Vec<Entry>>;
    fn delete_by_id(&mut self, id: u64) -> anyhow::Result<bool>;
    fn update(&mut self, id: u64, patch: &Entry) -> anyhow::Result<()>;
}
