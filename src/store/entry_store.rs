use crate::domain::Entry;

pub trait EntryStore {
    fn append(&mut self, entry: &Entry) -> anyhow::Result<()>;
    fn write_all(&mut self, entries: &[Entry]) -> anyhow::Result<()>;
    fn read_all(&self) -> anyhow::Result<(u64, Vec<Entry>)>;
    fn delete_by_id(&mut self, id: u64) -> anyhow::Result<bool>;
}
