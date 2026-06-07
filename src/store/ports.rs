use crate::domain::{ChangeOutcome, RecordChange, RecordSet};

pub(crate) trait RecordRepository {
    fn records(&self) -> &RecordSet;
    fn commit(&mut self, change: RecordChange) -> anyhow::Result<ChangeOutcome>;
}
