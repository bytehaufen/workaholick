use anyhow::Context;
use indexmap::IndexMap;

use crate::domain::{Record, RecordId, RecordInput};

pub(crate) enum RecordChange {
    Insert { id: RecordId, input: RecordInput },
    Update { id: RecordId, input: RecordInput },
    Delete { id: RecordId },
}

#[derive(Debug)]
pub(crate) enum ChangeOutcome {
    Inserted { id: RecordId },
    Updated { previous: Record },
    Deleted { record: Record },
}

#[derive(Debug, Clone)]
pub(crate) struct RecordSet(IndexMap<RecordId, Record>);

impl RecordSet {
    pub fn try_from_records(records: Vec<Record>) -> anyhow::Result<Self> {
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

    pub fn apply_change(&mut self, change: &RecordChange) -> anyhow::Result<ChangeOutcome> {
        match change {
            RecordChange::Insert { id, input } => {
                anyhow::ensure!(!self.0.contains_key(id), "id {id} already exists");
                let record = Record::from_input(*id, input.clone());
                self.insert_sorted(*id, record);

                Ok(ChangeOutcome::Inserted { id: *id })
            }
            RecordChange::Update { id, input } => {
                let existing = self.0.get(id).with_context(|| format!("id {id} does not exist"))?;
                anyhow::ensure!(
                    same_record_type(existing, input),
                    "record {id} cannot be updated with a different record type"
                );

                let updated_record = Record::from_input(*id, input.clone());
                match self.0.shift_remove(id) {
                    Some(previous) => {
                        self.insert_sorted(*id, updated_record);

                        Ok(ChangeOutcome::Updated { previous })
                    }
                    None => anyhow::bail!("id {id} does not exist"),
                }
            }
            RecordChange::Delete { id } => {
                // We do a slow shift remove, to do not mess the order in the stored file
                let record =
                    self.0.shift_remove(id).with_context(|| format!("id {id} does not exist"))?;

                Ok(ChangeOutcome::Deleted { record })
            }
        }
    }

    pub fn get(&self, id: &RecordId) -> Option<&Record> {
        self.0.get(id)
    }
    pub fn iter(&self) -> impl Iterator<Item = &Record> {
        self.0.values()
    }
    #[allow(unused)]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    #[allow(unused)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn insert_sorted(&mut self, id: RecordId, record: Record) {
        self.0.insert_sorted_by(id, record, |_i1, e1, _i2, e2| e1.sort_key().cmp(&e2.sort_key()));
    }

    pub(crate) fn next_id(&self) -> RecordId {
        self.0.keys().max().map(|id| id.incremented()).unwrap_or_default()
    }
}

fn same_record_type(record: &Record, input: &RecordInput) -> bool {
    matches!(
        (record, input),
        (Record::Work(_), RecordInput::Work(_)) | (Record::Absence(_), RecordInput::Absence(_))
    )
}
