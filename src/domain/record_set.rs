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

    fn insert_sorted(&mut self, id: RecordId, record: Record) {
        let _ = self
            .0
            .insert_sorted_by(id, record, |_i1, e1, _i2, e2| e1.sort_key().cmp(&e2.sort_key()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{AbsenceKind, AbsencePortion, WorkCommand, WorkContext},
        test_support::{absence_record, work_record},
    };

    #[test]
    fn try_from_sorts_by_sortkey() {
        let records = vec![
            work_record(42, 27, 5, 19, WorkCommand::Start { context: WorkContext::Home }),
            absence_record(37, 26, AbsenceKind::Holiday, AbsencePortion::FullDay),
            absence_record(127, 29, AbsenceKind::Vacation, AbsencePortion::HalfDay),
            absence_record(27, 28, AbsenceKind::Sick, AbsencePortion::HalfDay),
            work_record(74, 28, 5, 19, WorkCommand::Start { context: WorkContext::Home }),
            work_record(
                1,
                28,
                5,
                20,
                WorkCommand::End { title: "halfday".into(), description: None },
            ),
            absence_record(26, 30, AbsenceKind::Sick, AbsencePortion::HalfDay),
            work_record(
                41,
                27,
                5,
                20,
                WorkCommand::End { title: "fullday".into(), description: None },
            ),
            absence_record(5, 29, AbsenceKind::Sick, AbsencePortion::HalfDay),
            work_record(75, 30, 5, 19, WorkCommand::Start { context: WorkContext::Home }),
            absence_record(9, 31, AbsenceKind::Holiday, AbsencePortion::FullDay),
            work_record(
                4,
                30,
                5,
                20,
                WorkCommand::End { title: "halfday3".into(), description: None },
            ),
        ];

        let record_ids: Vec<RecordId> =
            RecordSet::try_from_records(records).unwrap().0.keys().copied().collect();

        let expected: Vec<RecordId> = vec![37, 42, 41, 27, 74, 1, 5, 127, 26, 75, 4, 9]
            .into_iter()
            .map(RecordId::from)
            .collect();

        assert_eq!(record_ids, expected);
    }
}
