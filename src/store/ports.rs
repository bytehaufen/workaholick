use crate::{
    domain::{ChangeOutcome, RecordChange, RecordSet},
    store::CommitError,
};

pub(crate) trait RecordRepository {
    fn records(&self) -> &RecordSet;
    fn commit(&mut self, change: RecordChange) -> Result<ChangeOutcome, CommitError>;
}
