use std::path::PathBuf;

use crate::domain::RecordSetError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CommitError {
    #[error(transparent)]
    RecordSet(#[from] RecordSetError),

    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum StoreError {
    #[error("could not read records from {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not write records to {path}")]
    Persist {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not replace {to} with temporary store file {from}")]
    Replace {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not serialize records for {path}")]
    Serialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("could not parse record at line {line} in {path}")]
    Parse {
        line: usize,
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("store path {path} has no parent directory")]
    MissingParentDirectory { path: PathBuf },

    #[error("could not create store directory {path}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not create lock file {path}")]
    CreateLock {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("store is already in use (lock file exists: {path})")]
    AlreadyLocked { path: PathBuf },

    #[error("records loaded from {path} are invalid")]
    InvalidRecords {
        path: PathBuf,
        #[source]
        source: RecordSetError,
    },
}
