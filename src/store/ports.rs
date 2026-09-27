use crate::{
    domain::{ChangeOutcome, RecordChange, RecordSet},
    store::CommitError,
};

pub(crate) trait RecordRepository {
    /// Returns the current committed records.
    fn records(&self) -> &RecordSet;

    /// Applies the changes atomically, validating the resulting state before committing.
    ///
    /// On success, all changes are persisted and reflected by `records()`. Returns one outcome per
    /// change, in input order.
    ///
    ///On error, both persisted records and the state exposed by `records()` remain unchanged.
    fn commit(&mut self, changes: &[RecordChange]) -> Result<Vec<ChangeOutcome>, CommitError>;
}
