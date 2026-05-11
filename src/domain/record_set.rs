use anyhow::Context;
use indexmap::IndexMap;

use crate::domain::{Record, RecordId, RecordInput};

pub(crate) enum RecordChange {
    Insert { id: RecordId, input: RecordInput },
    Update { id: RecordId, input: RecordInput },
    Delete { id: RecordId },
}

pub(crate) enum ChangeOutcome {
    Inserted { id: RecordId },
    Updated { previous: Record },
    Deleted { record: Record },
}

#[derive(Debug, Clone)]
pub(crate) struct RecordSet(IndexMap<RecordId, Record>);

impl RecordSet {
    pub(crate) fn try_from_records(records: Vec<Record>) -> anyhow::Result<Self> {
        let len = records.len();
        let mut records_by_id = records.into_iter().try_fold(
            IndexMap::with_capacity(len),
            |mut records_by_id, record| {
                let id = *record.id();
                if records_by_id.insert(id, record).is_some() {
                    anyhow::bail!("duplicate id {id} in records");
                }

                Ok(records_by_id)
            },
        )?;

        records_by_id.sort_by_key(|_, e| e.sort_key());

        Ok(Self(records_by_id))
    }

    pub(crate) fn apply_change(&mut self, change: RecordChange) -> anyhow::Result<ChangeOutcome> {
        match change {
            RecordChange::Insert { id, input } => {
                let record = Record::from_input(id, input);
                self.insert_sorted(id, record);

                Ok(ChangeOutcome::Inserted { id })
            }
            RecordChange::Update { id, input } => {
                anyhow::ensure!(self.0.contains_key(&id), "id {id} does not exist");
                let updated_record = Record::from_input(id, input);
                match self.0.shift_remove(&id) {
                    Some(previous) => {
                        self.insert_sorted(id, updated_record);

                        Ok(ChangeOutcome::Updated { previous })
                    }
                    None => anyhow::bail!("id {id} does not exist"),
                }
            }
            RecordChange::Delete { id } => {
                // We do a slow shift remove, to do not mess the order in the stored file
                let record =
                    self.0.shift_remove(&id).with_context(|| format!("id {id} does not exist"))?;

                Ok(ChangeOutcome::Deleted { record })
            }
        }
    }

    pub(crate) fn get(&self, id: &RecordId) -> Option<&Record> {
        self.0.get(id)
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Record> {
        self.0.values()
    }
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn insert_sorted(&mut self, id: RecordId, record: Record) {
        self.0.insert_sorted_by(id, record, |_i1, e1, _i2, e2| e1.sort_key().cmp(&e2.sort_key()));
    }

    pub(crate) fn next_id(&self) -> RecordId {
        self.0.keys().max().map(|id| id.incremented()).unwrap_or_default()
    }
}
