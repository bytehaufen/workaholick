use crate::domain::{ChangeOutcome, Record, RecordChange, RecordId, RecordInput, RecordSet};

use anyhow::Result;

pub trait RecordReader {
    /// Deterministic store order
    fn iter(&self) -> Box<dyn Iterator<Item = &Record> + '_>;
    fn get(&self, id: &RecordId) -> Option<&Record>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

pub trait RecordWriter {
    fn next_id(&self) -> RecordId;
    fn insert(&mut self, input: RecordInput) -> Result<RecordId>;
    fn update(&mut self, id: &RecordId, input: RecordInput) -> Result<()>;
    fn delete(&mut self, id: &RecordId) -> Result<Record>;
}

pub trait RecordStore: RecordReader + RecordWriter {}
impl<T: RecordReader + RecordWriter> RecordStore for T {}

pub trait RecordRepository {
    fn records(&self) -> &RecordSet;
    fn commit(&mut self, change: RecordChange) -> anyhow::Result<ChangeOutcome>;
}
