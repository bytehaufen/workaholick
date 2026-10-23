use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::domain::{ChangeOutcome, Record, RecordChange, RecordSet, validate_records};
use crate::store::{CommitError, RecordRepository, StoreError};

const BUFFER_SIZE: usize = 1024 * 1024; // 1 MB

#[derive(Debug)]
pub struct JsonlFileStore {
    file_path: PathBuf,
    _lock_file: File,
    records_by_id: RecordSet,
}

impl JsonlFileStore {
    pub fn new(file_path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let file_path: &Path = file_path.as_ref();
        let parent = file_path
            .parent()
            .ok_or_else(|| StoreError::MissingParentDirectory { path: file_path.to_path_buf() })?;

        fs::create_dir_all(parent)
            .map_err(|source| StoreError::CreateDirectory { path: parent.to_path_buf(), source })?;

        let lock_file = Self::create_lock_file(file_path)?;

        let records = Self::read_file(file_path)?;
        let records_by_id = RecordSet::try_from_records(records).map_err(|source| {
            StoreError::InvalidRecords { path: file_path.to_path_buf(), source }
        })?;

        Ok(Self { file_path: file_path.into(), _lock_file: lock_file, records_by_id })
    }

    fn read_from<R: BufRead>(reader: R, path: &Path) -> Result<Vec<Record>, StoreError> {
        reader
            .lines()
            .enumerate()
            .filter_map(|(index, line)| match line {
                Ok(line) if line.trim().is_empty() => None,
                other => Some((index, other)),
            })
            .map(|(index, line)| {
                let line =
                    line.map_err(|source| StoreError::Read { path: path.to_path_buf(), source })?;
                let line_num = index + 1;

                serde_json::from_str::<Record>(&line).map_err(|source| StoreError::Parse {
                    line: line_num,
                    path: path.to_path_buf(),
                    source,
                })
            })
            .collect()
    }

    /// Reads in the file at `path` in a by timeestamp sorted `Vec<Record>`
    fn read_file(path: &Path) -> Result<Vec<Record>, StoreError> {
        if !path.exists() {
            return Ok(Vec::new());
        };

        let file = File::open(path)
            .map_err(|source| StoreError::Read { path: path.to_path_buf(), source })?;

        let reader = BufReader::with_capacity(BUFFER_SIZE, file);
        Self::read_from(reader, path)
    }

    fn persist(&self, candidate: &RecordSet) -> Result<(), StoreError> {
        let parent = self
            .file_path
            .parent()
            .ok_or_else(|| StoreError::MissingParentDirectory { path: self.file_path.clone() })?;

        let temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|source| StoreError::Persist { path: self.file_path.clone(), source })?;

        {
            let mut writer = BufWriter::with_capacity(BUFFER_SIZE, temporary.as_file());

            candidate.iter().try_for_each(|record| -> Result<(), StoreError> {
                serde_json::to_writer(&mut writer, record).map_err(|source| {
                    StoreError::Serialize { path: temporary.path().to_path_buf(), source }
                })?;
                writeln!(writer).map_err(|source| StoreError::Persist {
                    path: temporary.path().to_path_buf(),
                    source,
                })?;
                Ok(())
            })?;

            writer.flush().map_err(|source| StoreError::Persist {
                path: temporary.path().to_path_buf(),
                source,
            })?;
        }

        temporary.as_file().sync_all().map_err(|source| StoreError::Persist {
            path: temporary.path().to_path_buf(),
            source,
        })?;

        let _file = temporary.persist(&self.file_path).map_err(|error| StoreError::Replace {
            from: error.file.path().to_path_buf(),
            to: self.file_path.to_path_buf(),
            source: error.error,
        })?;

        Ok(())
    }

    fn create_lock_file(file_path: &Path) -> Result<File, StoreError> {
        let path = file_path.with_added_extension("lock");

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|source| StoreError::CreateLock { path: path.clone(), source })?;

        match file.try_lock() {
            Ok(()) => Ok(file),
            Err(TryLockError::WouldBlock) => Err(StoreError::AlreadyLocked { path }),
            Err(TryLockError::Error(source)) => Err(StoreError::AcquireLock { path, source }),
        }
    }
}

impl RecordRepository for JsonlFileStore {
    fn records(&self) -> &RecordSet {
        &self.records_by_id
    }

    fn commit(&mut self, changes: &[RecordChange]) -> Result<Vec<ChangeOutcome>, CommitError> {
        let mut candidate = self.records_by_id.clone();

        let outcomes = changes
            .iter()
            .map(|change| candidate.apply_change(change))
            .collect::<Result<Vec<_>, _>>()?;
        validate_records(&candidate)?;
        self.persist(&candidate)?;
        self.records_by_id = candidate;

        Ok(outcomes)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Local, TimeZone, Timelike};
    use core::slice;
    use std::path::Path;

    use crate::{
        domain::{
            AbsenceKind, AbsencePortion, RecordId, RecordInput, RecordSetError, WorkCommand,
            WorkContext, WorkRecord, WorkRecordInput,
        },
        test_support::{absence_record, date, ts, work_record},
    };

    use super::*;

    fn work_input(timestamp: DateTime<Local>, command: WorkCommand) -> RecordInput {
        RecordInput::Work(WorkRecordInput::new(timestamp, command))
    }

    fn to_work_record(record: &Record) -> &WorkRecord {
        record.as_work().expect("expected work record")
    }

    fn write_records(path: &Path, records: &[Record]) {
        let mut file = File::create(path).unwrap();
        for record in records {
            serde_json::to_writer(&mut file, record).unwrap();
            writeln!(file).unwrap();
        }
        file.flush().unwrap();
    }

    #[test]
    fn commit_insert_persists_and_returns_inserted_id() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            let input = work_input(ts(6, 6, 19), WorkCommand::Start { context: WorkContext::Home });

            let outcomes = store.commit(&[RecordChange::Insert { input }]).unwrap();

            assert_eq!(outcomes, vec![ChangeOutcome::Inserted { id: 0.into() }]);
        }

        let store = JsonlFileStore::new(&file_path).unwrap();

        let expected = work_record(0, 6, 6, 19, WorkCommand::Start { context: WorkContext::Home });

        assert_eq!(store.records().len(), 1);
        assert_eq!(store.records().get(&0.into()).unwrap(), &expected);
    }

    #[test]
    fn commit_delete_persists_and_returns_deleted_record() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        {
            let work_record =
                work_record(0, 6, 6, 25, WorkCommand::Start { context: WorkContext::Home });
            write_records(&file_path, slice::from_ref(&work_record));

            let mut store = JsonlFileStore::new(&file_path).unwrap();
            assert!(store.records().get(&0.into()).is_some());

            let outcomes = store.commit(&[RecordChange::Delete { id: 0.into() }]).unwrap();

            assert_eq!(outcomes, vec![ChangeOutcome::Deleted { record: work_record }]);
        }

        let store = JsonlFileStore::new(&file_path).unwrap();

        assert_eq!(store.records().len(), 0);
        assert!(store.records().get(&0.into()).is_none());
    }

    #[test]
    fn commit_updates_persists_and_returns_previous_record() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        let input = work_input(ts(6, 6, 50), WorkCommand::Start { context: WorkContext::Office });

        {
            let work_record =
                work_record(0, 6, 6, 48, WorkCommand::Start { context: WorkContext::Home });
            write_records(&file_path, slice::from_ref(&work_record));

            let mut store = JsonlFileStore::new(&file_path).unwrap();
            assert!(store.records().get(&0.into()).is_some());
            let outcomes = store
                .commit(&[RecordChange::Update { id: 0.into(), input: input.clone() }])
                .unwrap();

            assert_eq!(outcomes, vec![ChangeOutcome::Updated { previous: work_record }]);
        }

        let store = JsonlFileStore::new(&file_path).unwrap();

        assert_eq!(store.records().len(), 1);
        assert_eq!(store.records().get(&0.into()).unwrap(), &Record::from_input(0.into(), input));
    }

    #[test]
    fn insert_persists_and_reopen_reads_records() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            let _ = store
                .commit(&[
                    RecordChange::Insert {
                        input: work_input(
                            Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap(),
                            WorkCommand::Start { context: WorkContext::Office },
                        ),
                    },
                    RecordChange::Insert {
                        input: work_input(
                            Local.with_ymd_and_hms(2026, 2, 3, 4, 5, 6).unwrap(),
                            WorkCommand::End {
                                title: "The main thing".into(),
                                description: Some("did some stuff".into()),
                            },
                        ),
                    },
                ])
                .unwrap();
        }

        let store = JsonlFileStore::new(&file_path).unwrap();
        assert_eq!(store.records_by_id.len(), 2);
    }

    #[test]
    fn delete_by_id_persists() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        {
            write_records(
                &file_path,
                &[
                    work_record(0, 2, 3, 4, WorkCommand::Start { context: WorkContext::Home }),
                    work_record(
                        1,
                        3,
                        4,
                        5,
                        WorkCommand::Log {
                            title: "the a thing".to_string(),
                            description: Some("a".into()),
                        },
                    ),
                    work_record(
                        2,
                        4,
                        5,
                        6,
                        WorkCommand::End { title: "the b thing".to_string(), description: None },
                    ),
                ],
            );

            let mut store = JsonlFileStore::new(&file_path).unwrap();

            let work_record = match store
                .commit(&[RecordChange::Delete { id: 1.into() }])
                .unwrap()
                .into_iter()
                .next()
            {
                Some(ChangeOutcome::Deleted { record: Record::Work(work_record) }) => work_record,
                _ => panic!("expected work record"),
            };

            assert_eq!(
                *work_record.command(),
                WorkCommand::Log { title: "the a thing".into(), description: Some("a".into()) }
            );
        }

        let store = JsonlFileStore::new(&file_path).unwrap();
        assert_eq!(store.records_by_id.len(), 2);
    }

    #[test]
    fn update_record_reorders_by_sort_key_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        {
            write_records(
                &file_path,
                &[
                    work_record(1, 24, 17, 0, WorkCommand::Start { context: WorkContext::Home }),
                    work_record(
                        2,
                        24,
                        19,
                        0,
                        WorkCommand::Log { title: "The a thing".into(), description: None },
                    ),
                    work_record(
                        3,
                        24,
                        20,
                        0,
                        WorkCommand::Log { title: "The b thing".into(), description: None },
                    ),
                    work_record(
                        4,
                        24,
                        21,
                        0,
                        WorkCommand::End { title: "The c thing".into(), description: None },
                    ),
                ],
            );
        }

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            let _ = store
                .commit(&[RecordChange::Update {
                    id: 3.into(),
                    input: work_input(
                        ts(24, 18, 0),
                        WorkCommand::Log { title: "The new B thing".into(), description: None },
                    ),
                }])
                .unwrap();
        }

        let store = JsonlFileStore::new(&file_path).unwrap();
        assert_eq!(store.records_by_id.len(), 4);
        let ids = store.records().iter().map(|record| *record.id()).collect::<Vec<_>>();
        assert_eq!(ids, vec![1.into(), 3.into(), 2.into(), 4.into()]);
        assert_eq!(
            to_work_record(store.records_by_id.get(&3.into()).unwrap()).timestamp().hour(),
            18
        );
    }

    #[test]
    fn update_and_delete_missing_id_return_error() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");
        let mut store = JsonlFileStore::new(&file_path).unwrap();

        let _ = store.commit(&[RecordChange::Insert {
            input: work_input(
                Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                WorkCommand::Log { title: "the a thing".into(), description: Some("a".into()) },
            ),
        }]);

        let update_err = store
            .commit(&[RecordChange::Update {
                id: 999.into(),
                input: work_input(
                    Local.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                    WorkCommand::Log { title: "the b thing".into(), description: Some("b".into()) },
                ),
            }])
            .unwrap_err();
        assert_eq!(update_err.to_string(), "record with id 999 does not exist");

        let delete_err = store.commit(&[RecordChange::Delete { id: 123.into() }]).unwrap_err();
        assert_eq!(delete_err.to_string(), "record with id 123 does not exist");
    }

    #[test]
    fn insert_after_reopen_continues_ids() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            let _ = store
                .commit(&[
                    RecordChange::Insert {
                        input: work_input(
                            ts(1, 1, 0),
                            WorkCommand::Start { context: WorkContext::Office },
                        ),
                    },
                    RecordChange::Insert {
                        input: work_input(
                            ts(1, 2, 0),
                            WorkCommand::Log { title: "a".into(), description: None },
                        ),
                    },
                    RecordChange::Insert {
                        input: work_input(
                            ts(1, 3, 0),
                            WorkCommand::Log { title: "b".into(), description: None },
                        ),
                    },
                ])
                .unwrap();
        }

        let mut store = JsonlFileStore::new(&file_path).unwrap();
        let _ = store
            .commit(&[RecordChange::Insert {
                input: work_input(
                    ts(1, 4, 0),
                    WorkCommand::Log { title: "c".into(), description: None },
                ),
            }])
            .unwrap();

        assert_eq!(store.records_by_id.len(), 4);
    }

    #[test]
    fn iter_returns_records_sorted_by_sort_key() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        let records = vec![
            Record::from_input(
                1.into(),
                work_input(
                    Local.with_ymd_and_hms(2026, 1, 5, 4, 30, 26).unwrap(),
                    WorkCommand::Start { context: WorkContext::Home },
                ),
            ),
            Record::from_input(
                2.into(),
                work_input(
                    Local.with_ymd_and_hms(2026, 1, 5, 3, 45, 3).unwrap(),
                    WorkCommand::End { title: "foo".into(), description: None },
                ),
            ),
        ];
        write_records(&file_path, &records);

        let store = JsonlFileStore::new(&file_path).unwrap();
        let ids: Vec<_> = store.records().iter().map(|record| *record.id()).collect();
        assert_eq!(ids, vec![RecordId::from(2), RecordId::from(1)]);
    }

    #[test]
    fn get_returns_record_for_existing_id_and_none_for_missing_id() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");
        let mut store = JsonlFileStore::new(&file_path).unwrap();

        let outcomes = store
            .commit(&[RecordChange::Insert {
                input: work_input(ts(1, 9, 0), WorkCommand::Start { context: WorkContext::Office }),
            }])
            .unwrap();

        assert_eq!(outcomes, vec![ChangeOutcome::Inserted { id: 0.into() }]);

        let expected = work_record(0, 1, 9, 0, WorkCommand::Start { context: WorkContext::Office });
        assert_eq!(store.records().get(&0.into()), Some(&expected));
        assert!(store.records().get(&999.into()).is_none());
    }

    #[test]
    fn new_rejects_path_without_parent_directory() {
        let file_path = PathBuf::from("/");
        let err = JsonlFileStore::new(&file_path).unwrap_err();
        assert_eq!(err.to_string(), "store path / has no parent directory");
    }

    #[test]
    #[cfg(unix)]
    fn new_adds_context_when_opening_existing_file_fails() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        let _ = File::create(&file_path).unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o000);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let err = JsonlFileStore::new(&file_path).unwrap_err().to_string();
        assert!(err.contains("could not read records from"));

        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&file_path, perms).unwrap();
    }

    #[test]
    fn persist_adds_context_when_rename_fails() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        let mut store = JsonlFileStore::new(&file_path).unwrap();
        std::fs::create_dir(&file_path).unwrap();

        let err = store
            .commit(&[RecordChange::Insert {
                input: work_input(
                    Local.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                    WorkCommand::Start { context: WorkContext::Home },
                ),
            }])
            .unwrap_err()
            .to_string();

        assert!(err.contains("could not replace"));
    }

    #[test]
    #[cfg(unix)]
    fn commit_preserves_memory_and_disk_when_persistence_fails() {
        use std::{fs::Permissions, io::ErrorKind, os::unix::fs::PermissionsExt};

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");
        let mut store = JsonlFileStore::new(&file_path).unwrap();

        let _ = store
            .commit(&[RecordChange::Insert {
                input: RecordInput::new_absence(
                    date(10),
                    AbsenceKind::Vacation,
                    AbsencePortion::FullDay,
                    None,
                ),
            }])
            .unwrap();

        let original_records = store.records().iter().cloned().collect::<Vec<_>>();
        let original_bytes = std::fs::read(&file_path).unwrap();

        let changes = [
            RecordChange::Insert {
                input: RecordInput::new_absence(
                    date(11),
                    AbsenceKind::Holiday,
                    AbsencePortion::FullDay,
                    None,
                ),
            },
            RecordChange::Insert {
                input: RecordInput::new_absence(
                    date(12),
                    AbsenceKind::Holiday,
                    AbsencePortion::FullDay,
                    None,
                ),
            },
        ];

        let original_permissions = std::fs::metadata(dir.path()).unwrap().permissions();
        std::fs::set_permissions(dir.path(), Permissions::from_mode(0o500)).unwrap();

        if tempfile::NamedTempFile::new_in(dir.path()).is_ok() {
            std::fs::set_permissions(dir.path(), original_permissions).unwrap();
            eprintln!("Skipping: this user can write to a read-only directory.");

            return;
        }

        let result = store.commit(&changes);
        std::fs::set_permissions(dir.path(), original_permissions).unwrap();

        let err = result.unwrap_err();

        assert!(
            matches!(err, CommitError::Store(StoreError::Persist { path, source }) if path == file_path &&source.kind() == ErrorKind::PermissionDenied)
        );
        assert_eq!(store.records().iter().cloned().collect::<Vec<_>>(), original_records);
        assert_eq!(std::fs::read(&file_path).unwrap(), original_bytes);

        drop(store);

        let reopened = JsonlFileStore::new(&file_path).unwrap();
        assert_eq!(reopened.records().iter().cloned().collect::<Vec<_>>(), original_records);
    }

    #[test]
    fn lock_file_prevents_second_instance_and_is_removed_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        {
            let _store1 = JsonlFileStore::new(&file_path).unwrap();
            let err = JsonlFileStore::new(&file_path).unwrap_err();
            assert!(err.to_string().contains("lock file exists"));
        }

        let _store2 = JsonlFileStore::new(&file_path).unwrap();
    }

    #[test]
    fn lock_is_released_if_new_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.jsonl");

        std::fs::write(&path, "invalid jason").unwrap();

        let error = JsonlFileStore::new(&path).unwrap_err();
        assert!(matches!(error, StoreError::Parse { .. }));

        std::fs::write(&path, "").unwrap();
        assert!(JsonlFileStore::new(&path).is_ok());
    }

    #[test]
    fn read_file_ignores_blank_lines() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        let record = Record::from_input(
            RecordId::from(0),
            work_input(
                Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap(),
                WorkCommand::Log { title: "the a thing".into(), description: Some("a".into()) },
            ),
        );

        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file).unwrap();
        serde_json::to_writer(&mut file, &record).unwrap();
        writeln!(file).unwrap();
        writeln!(file).unwrap();

        let store = JsonlFileStore::new(&file_path).unwrap();
        assert_eq!(store.records_by_id.len(), 1);
        assert_eq!(store.records_by_id.get(&0.into()).unwrap().id(), &RecordId::from(0));
    }

    #[test]
    fn read_file_reports_line_number_on_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "{{ this is not json }}").unwrap();

        let err = JsonlFileStore::new(&file_path).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("could not parse record at line 1 in "));
    }

    #[test]
    fn new_with_duplicate_ids_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let store_file_path = dir.path().join("data.jsonl");

        write_records(
            &store_file_path,
            &[
                Record::from_input(
                    0.into(),
                    work_input(
                        Local.with_ymd_and_hms(2026, 1, 5, 4, 30, 26).unwrap(),
                        WorkCommand::Start { context: WorkContext::Home },
                    ),
                ),
                Record::from_input(
                    0.into(),
                    work_input(
                        Local.with_ymd_and_hms(2026, 1, 5, 16, 45, 3).unwrap(),
                        WorkCommand::End { title: "foo".into(), description: None },
                    ),
                ),
            ],
        );

        let error = JsonlFileStore::new(&store_file_path).unwrap_err();
        assert!(matches!(
            error,
            StoreError::InvalidRecords {
                path,
                source: RecordSetError::DuplicateId { id },
            } if path == store_file_path && id == 0.into()
        ));
    }

    #[test]
    #[cfg(unix)]
    fn create_lock_file_with_permission_error() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();

        let lock_dir = dir.path().join("readonly");
        std::fs::create_dir(&lock_dir).unwrap();
        let mut perms = std::fs::metadata(&lock_dir).unwrap().permissions();
        perms.set_mode(0o444);
        std::fs::set_permissions(&lock_dir, perms).unwrap();

        let store_in_readonly = lock_dir.join("data.jsonl");
        let err = JsonlFileStore::new(&store_in_readonly).unwrap_err();

        let mut perms = std::fs::metadata(&lock_dir).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&lock_dir, perms).unwrap();

        let msg = err.to_string();
        assert!(msg.contains("could not create lock file "));
    }

    #[test]
    fn new_with_unsorted_records_normalizes_iteration_order() {
        let dir = tempfile::tempdir().unwrap();
        let store_file_path = dir.path().join("data.jsonl");

        write_records(
            &store_file_path,
            &[
                Record::from_input(
                    1.into(),
                    work_input(
                        Local.with_ymd_and_hms(2026, 1, 4, 4, 30, 26).unwrap(),
                        WorkCommand::Start { context: WorkContext::Home },
                    ),
                ),
                Record::from_input(
                    0.into(),
                    work_input(
                        Local.with_ymd_and_hms(2026, 1, 4, 4, 30, 26).unwrap(),
                        WorkCommand::Start { context: WorkContext::Home },
                    ),
                ),
                Record::from_input(
                    2.into(),
                    work_input(
                        Local.with_ymd_and_hms(2026, 1, 3, 4, 30, 26).unwrap(),
                        WorkCommand::Start { context: WorkContext::Home },
                    ),
                ),
                Record::from_input(
                    3.into(),
                    work_input(
                        Local.with_ymd_and_hms(2026, 1, 2, 16, 45, 3).unwrap(),
                        WorkCommand::Start { context: WorkContext::Home },
                    ),
                ),
            ],
        );

        let store = JsonlFileStore::new(&store_file_path).unwrap();
        let ids: Vec<_> = store.records().iter().map(|record| *record.id()).collect();
        assert_eq!(ids, [3, 2, 0, 1].into_iter().map(RecordId::from).collect::<Vec<_>>());
    }

    #[test]
    fn len_and_is_empty_reflect_store_state() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        let mut store = JsonlFileStore::new(&file_path).unwrap();
        assert_eq!(store.records().len(), 0);
        assert!(store.records().is_empty());

        let change_outcome = store
            .commit(&[RecordChange::Insert {
                input: work_input(ts(15, 5, 15), WorkCommand::Start { context: WorkContext::Home }),
            }])
            .unwrap();
        assert_eq!(store.records().len(), 1);
        assert!(!store.records().is_empty());

        let Some(ChangeOutcome::Inserted { id }) = change_outcome.first() else {
            panic!("expected inserted outcome")
        };

        let _ = store.commit(&[RecordChange::Delete { id: *id }]).unwrap();
        assert_eq!(store.records().len(), 0);
        assert!(store.records().is_empty());
    }

    #[test]
    fn new_reports_parent_path_when_directory_creation_fails() {
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("not-a-directory");
        std::fs::write(&parent, "blocking file").unwrap();

        let store_path = parent.join("data.jsonl");
        let err = JsonlFileStore::new(&store_path).unwrap_err();

        assert!(matches!(err, StoreError::CreateDirectory { path, .. } if path == parent));
    }

    #[test]
    fn insert_rejects_invalid_work_timeline_mutations() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        write_records(
            &file_path,
            &[
                work_record(1, 11, 5, 10, WorkCommand::Start { context: WorkContext::Home }),
                work_record(2, 11, 5, 11, WorkCommand::BreakStart),
            ],
        );
        let mut store = JsonlFileStore::new(&file_path).unwrap();
        let now = ts(11, 5, 15);

        let err = store
            .commit(&[RecordChange::Insert {
                input: RecordInput::new_work(
                    now,
                    WorkCommand::Log {
                        title: "refactoring round  42".to_string(),
                        description: None,
                    },
                ),
            }])
            .unwrap_err();

        assert_eq!(err.to_string(), "cannot log while breaking for 2026-01-11");
        assert_eq!(store.records().iter().count(), 2);
    }

    #[test]
    fn update_rejects_invalid_work_timeline_permutations() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        write_records(
            &file_path,
            &[
                work_record(1, 11, 8, 5, WorkCommand::Start { context: WorkContext::Home }),
                work_record(2, 11, 8, 6, WorkCommand::BreakStart),
            ],
        );

        let mut store = JsonlFileStore::new(&file_path).unwrap();

        let update = RecordChange::Update {
            id: 2.into(),
            input: RecordInput::new_work(
                ts(11, 8, 6),
                WorkCommand::Start { context: WorkContext::Home },
            ),
        };

        let err = store.commit(&[update]).unwrap_err();

        assert!(matches!(err, CommitError::TimelineValidation(_)));

        let expected = store.records().get(&2.into()).unwrap().as_work().unwrap().command();
        assert!(matches!(expected, WorkCommand::BreakStart));
    }

    #[test]
    fn delete_rejects_invalid_work_timeline_permutations() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        write_records(
            &file_path,
            &[
                work_record(1, 11, 8, 25, WorkCommand::Start { context: WorkContext::Home }),
                work_record(2, 11, 8, 26, WorkCommand::BreakStart),
            ],
        );
        let mut store = JsonlFileStore::new(&file_path).unwrap();

        let err = store.commit(&[RecordChange::Delete { id: 1.into() }]).unwrap_err();

        assert!(matches!(err, CommitError::TimelineValidation(_)));

        let expected = store.records().get(&1.into()).unwrap().as_work().unwrap().command();
        assert!(matches!(expected, WorkCommand::Start { context: WorkContext::Home }));
    }

    #[test]
    fn commit_rejects_conflicting_absence_batch_without_changing_state() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");
        write_records(
            &file_path,
            &[absence_record(42, 24, AbsenceKind::Vacation, AbsencePortion::FullDay)],
        );

        let mut store = JsonlFileStore::new(&file_path).unwrap();
        let err = store
            .commit(&[
                RecordChange::Insert {
                    input: RecordInput::new_absence(
                        date(23),
                        AbsenceKind::Holiday,
                        AbsencePortion::FullDay,
                        None,
                    ),
                },
                RecordChange::Insert {
                    input: RecordInput::new_absence(
                        date(24),
                        AbsenceKind::Holiday,
                        AbsencePortion::FullDay,
                        None,
                    ),
                },
            ])
            .unwrap_err();

        assert!(matches!(err, CommitError::TimelineValidation(_)));

        let expected = &[&absence_record(42, 24, AbsenceKind::Vacation, AbsencePortion::FullDay)];

        assert_eq!(&store.records().iter().collect::<Vec<_>>(), expected);
        drop(store);

        let store = JsonlFileStore::new(&file_path).unwrap();
        assert_eq!(&store.records().iter().collect::<Vec<_>>(), expected);
    }
}
