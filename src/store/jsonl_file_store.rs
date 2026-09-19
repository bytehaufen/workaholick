use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};

use crate::domain::{ChangeOutcome, Record, RecordChange, RecordSet};
use crate::store::{CommitError, RecordRepository, StoreError};

const BUFFER_SIZE: usize = 1024 * 1024; // 1 MB

#[derive(Debug)]
pub struct JsonlFileStore {
    file_path: PathBuf,
    _lock_file: File,
    lock_path: PathBuf,
    records_by_id: RecordSet,
}

impl Drop for JsonlFileStore {
    fn drop(&mut self) {
        // Lock-file is auto dropped, so we only remove it
        let _ = fs::remove_file(&self.lock_path);
    }
}

impl JsonlFileStore {
    pub fn new(file_path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let file_path: &Path = file_path.as_ref();
        let parent = file_path
            .parent()
            .ok_or_else(|| StoreError::MissingParentDirectory { path: file_path.to_path_buf() })?;

        fs::create_dir_all(parent)
            .map_err(|source| StoreError::CreateDirectory { path: parent.to_path_buf(), source })?;

        let (lock_file, lock_path) = Self::create_lock_file(file_path)?;

        let try_load = || {
            let records = Self::read_file(file_path)?;
            RecordSet::try_from_records(records).map_err(|source| StoreError::InvalidRecords {
                path: file_path.to_path_buf(),
                source,
            })
        };

        let records_by_id = match try_load() {
            Ok(records_by_id) => records_by_id,
            Err(error) => {
                drop(lock_file);
                let _ = fs::remove_file(&lock_path);
                return Err(error);
            }
        };

        Ok(Self { file_path: file_path.into(), _lock_file: lock_file, lock_path, records_by_id })
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

    fn replace_file(&self, from: &Path, to: &Path) -> Result<(), StoreError> {
        #[cfg(windows)]
        {
            if to.exists() {
                let _ = fs::remove_file(to);
            }
        }

        fs::rename(from, to).map_err(|source| StoreError::Replace {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            source,
        })?;
        Ok(())
    }

    fn temp_file_path(path: &Path) -> PathBuf {
        path.with_added_extension("tmp")
    }

    fn persist(&mut self) -> Result<(), StoreError> {
        let file_path = &self.file_path;
        let temp_file = Self::temp_file_path(file_path);

        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp_file)
            .map_err(|source| StoreError::Persist { path: temp_file.clone(), source })?;

        let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);

        self.records_by_id.iter().try_for_each(|record| -> Result<(), StoreError> {
            serde_json::to_writer(&mut writer, record)
                .map_err(|source| StoreError::Serialize { path: temp_file.clone(), source })?;
            writeln!(writer)
                .map_err(|source| StoreError::Persist { path: temp_file.clone(), source })?;
            Ok(())
        })?;

        writer.flush().map_err(|source| StoreError::Persist { path: temp_file.clone(), source })?;
        writer
            .get_ref()
            .sync_all()
            .map_err(|source| StoreError::Persist { path: temp_file.clone(), source })?;

        self.replace_file(&temp_file, &self.file_path)
    }

    fn create_lock_file(file_path: &Path) -> Result<(File, PathBuf), StoreError> {
        let path = file_path.with_added_extension("lock");
        let file = File::create_new(&path).map_err(|source| {
            if source.kind() == ErrorKind::AlreadyExists {
                StoreError::AlreadyLocked { path: path.clone() }
            } else {
                StoreError::CreateLock { path: path.clone(), source }
            }
        })?;

        Ok((file, path))
    }
}

impl RecordRepository for JsonlFileStore {
    fn records(&self) -> &RecordSet {
        &self.records_by_id
    }

    fn commit(&mut self, change: RecordChange) -> Result<ChangeOutcome, CommitError> {
        let change_outcome = self.records_by_id.apply_change(&change)?;
        self.persist()?;
        Ok(change_outcome)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone, Timelike};
    use core::slice;
    use std::path::Path;

    use crate::{
        domain::{
            RecordId, RecordInput, RecordSetError, WorkCommand, WorkContext, WorkRecord,
            WorkRecordInput,
        },
        test_support::{self, ts},
    };

    use super::*;

    fn work_input(timestamp: chrono::DateTime<Local>, command: WorkCommand) -> RecordInput {
        RecordInput::Work(WorkRecordInput::new(timestamp, command))
    }

    fn work_record(record: &Record) -> &WorkRecord {
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

    impl JsonlFileStore {
        fn next_id(&self) -> RecordId {
            self.records().next_id()
        }
    }

    #[test]
    fn commit_insert_persists_and_returns_inserted_id() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            let input = work_input(ts(6, 6, 19), WorkCommand::Start { context: WorkContext::Home });

            let outcome = store.commit(RecordChange::Insert { id: 0.into(), input }).unwrap();

            assert!(matches!(outcome, ChangeOutcome::Inserted { id}  if id == 0.into()));
        }

        let store = JsonlFileStore::new(&file_path).unwrap();

        assert_eq!(store.records().len(), 1);
        assert!(store.records().get(&0.into()).is_some());
    }

    #[test]
    fn commit_delete_persists_and_returns_deleted_record() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        {
            let work_record = test_support::work_record(
                0,
                6,
                6,
                25,
                WorkCommand::Start { context: WorkContext::Home },
            );
            write_records(&file_path, slice::from_ref(&work_record));

            let mut store = JsonlFileStore::new(&file_path).unwrap();
            assert!(store.records().get(&0.into()).is_some());

            let outcome = store.commit(RecordChange::Delete { id: 0.into() }).unwrap();

            assert!(matches!(outcome, ChangeOutcome::Deleted { record}  if record  == work_record));
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
            let work_record = test_support::work_record(
                0,
                6,
                6,
                48,
                WorkCommand::Start { context: WorkContext::Home },
            );
            write_records(&file_path, slice::from_ref(&work_record));

            let mut store = JsonlFileStore::new(&file_path).unwrap();
            assert!(store.records().get(&0.into()).is_some());
            let outcome =
                store.commit(RecordChange::Update { id: 0.into(), input: input.clone() }).unwrap();

            assert!(
                matches!(outcome, ChangeOutcome::Updated { previous}  if previous  == work_record)
            );
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
                .commit(RecordChange::Insert {
                    id: store.next_id(),
                    input: work_input(
                        Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap(),
                        WorkCommand::Start { context: WorkContext::Office },
                    ),
                })
                .unwrap();
            let _ = store
                .commit(RecordChange::Insert {
                    id: store.next_id(),
                    input: work_input(
                        Local.with_ymd_and_hms(2026, 2, 3, 4, 5, 6).unwrap(),
                        WorkCommand::End {
                            title: "The main thing".into(),
                            description: Some("did some stuff".into()),
                        },
                    ),
                })
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
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            let _ = store
                .commit(RecordChange::Insert {
                    id: store.next_id(),
                    input: work_input(
                        Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap(),
                        WorkCommand::Log {
                            title: "the a thing".into(),
                            description: Some("a".into()),
                        },
                    ),
                })
                .unwrap();
            let _ = store
                .commit(RecordChange::Insert {
                    id: store.next_id(),
                    input: work_input(
                        Local.with_ymd_and_hms(2026, 2, 3, 4, 5, 6).unwrap(),
                        WorkCommand::Log {
                            title: "the b thing".into(),
                            description: Some("b".into()),
                        },
                    ),
                })
                .unwrap();
            let _ = store
                .commit(RecordChange::Insert {
                    id: store.next_id(),
                    input: work_input(
                        Local.with_ymd_and_hms(2027, 3, 4, 5, 6, 7).unwrap(),
                        WorkCommand::Log {
                            title: "the c thing".into(),
                            description: Some("c".into()),
                        },
                    ),
                })
                .unwrap();

            let work_record = match store.commit(RecordChange::Delete { id: 1.into() }).unwrap() {
                ChangeOutcome::Deleted { record: Record::Work(work_record) } => work_record,
                _ => panic!("expected work record"),
            };

            assert_eq!(
                *work_record.command(),
                WorkCommand::Log { title: "the b thing".into(), description: Some("b".into()) }
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
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            let _ = store
                .commit(RecordChange::Insert {
                    id: store.next_id(),
                    input: work_input(
                        Local.with_ymd_and_hms(2025, 12, 24, 17, 0, 0).unwrap(),
                        WorkCommand::Start { context: WorkContext::Home },
                    ),
                })
                .unwrap();
            let _ = store
                .commit(RecordChange::Insert {
                    id: store.next_id(),
                    input: work_input(
                        Local.with_ymd_and_hms(2025, 12, 24, 14, 0, 0).unwrap(),
                        WorkCommand::Log {
                            title: "So hungry".into(),
                            description: Some("eating potato salad".into()),
                        },
                    ),
                })
                .unwrap();
            let _ = store
                .commit(RecordChange::Insert {
                    id: store.next_id(),
                    input: work_input(
                        Local.with_ymd_and_hms(2025, 12, 24, 20, 0, 0).unwrap(),
                        WorkCommand::End {
                            title: "So excited".into(),
                            description: Some("unwrapping presents".into()),
                        },
                    ),
                })
                .unwrap();
        }

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            let _ = store
                .commit(RecordChange::Update {
                    id: 1.into(),
                    input: work_input(
                        Local.with_ymd_and_hms(2025, 12, 24, 18, 0, 0).unwrap(),
                        WorkCommand::End {
                            title: "So excited".into(),
                            description: Some("un-wrapping presents".into()),
                        },
                    ),
                })
                .unwrap();
        }

        let store = JsonlFileStore::new(&file_path).unwrap();
        assert_eq!(store.records_by_id.len(), 3);
        assert_eq!(work_record(store.records_by_id.get(&1.into()).unwrap()).timestamp().hour(), 18);
    }

    #[test]
    fn update_and_delete_missing_id_return_error() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");
        let mut store = JsonlFileStore::new(&file_path).unwrap();

        let _ = store
            .commit(RecordChange::Insert {
                id: store.next_id(),
                input: work_input(
                    Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                    WorkCommand::Log { title: "the a thing".into(), description: Some("a".into()) },
                ),
            })
            .unwrap();

        let update_err = store
            .commit(RecordChange::Update {
                id: 999.into(),
                input: work_input(
                    Local.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                    WorkCommand::Log { title: "the b thing".into(), description: Some("b".into()) },
                ),
            })
            .unwrap_err();
        assert_eq!(update_err.to_string(), "record with id 999 does not exist");

        let delete_err = store.commit(RecordChange::Delete { id: 123.into() }).unwrap_err();
        assert_eq!(delete_err.to_string(), "record with id 123 does not exist");
    }

    #[test]
    fn insert_after_reopen_continues_ids() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            let _ = store
                .commit(RecordChange::Insert {
                    id: store.next_id(),
                    input: work_input(
                        Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                        WorkCommand::Log { title: "a".into(), description: None },
                    ),
                })
                .unwrap();
            let _ = store
                .commit(RecordChange::Insert {
                    id: store.next_id(),
                    input: work_input(
                        Local.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                        WorkCommand::Log { title: "b".into(), description: None },
                    ),
                })
                .unwrap();
        }

        let mut store = JsonlFileStore::new(&file_path).unwrap();
        let _ = store
            .commit(RecordChange::Insert {
                id: store.next_id(),
                input: work_input(
                    Local.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                    WorkCommand::Log { title: "c".into(), description: None },
                ),
            })
            .unwrap();

        assert_eq!(store.records_by_id.len(), 3);
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

        let ChangeOutcome::Inserted { id } = store
            .commit(RecordChange::Insert {
                id: store.next_id(),
                input: work_input(
                    Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                    WorkCommand::Log { title: "a".into(), description: Some("b".into()) },
                ),
            })
            .unwrap()
        else {
            panic!("expected insert change outcome");
        };

        let record = store.records().get(&id).expect("record should be present");
        assert_eq!(
            *work_record(record).command(),
            WorkCommand::Log { title: "a".into(), description: Some("b".into()) }
        );
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
            .commit(RecordChange::Insert {
                id: store.next_id(),
                input: work_input(
                    Local.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                    WorkCommand::Log { title: "boom".into(), description: None },
                ),
            })
            .unwrap_err()
            .to_string();

        assert!(err.contains("could not replace"));
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
    fn lock_file_is_removed_in_new_if_err() {
        let dir = tempfile::tempdir().unwrap();
        let store_file_path = dir.path().join("data.jsonl");
        let lock_file_path = store_file_path.with_added_extension("lock");

        {
            let _store = JsonlFileStore::new(&store_file_path).unwrap();
            assert!(fs::exists(&lock_file_path).unwrap());
        }

        let mut store_file = fs::File::create_new(&store_file_path).unwrap();
        store_file.write_all("not parsable data".as_bytes()).unwrap();

        let _ = JsonlFileStore::new(&store_file_path).unwrap_err();

        assert!(!fs::exists(&lock_file_path).unwrap());
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
            .commit(RecordChange::Insert {
                id: store.next_id(),
                input: work_input(
                    Local.with_ymd_and_hms(2026, 3, 15, 5, 15, 0).unwrap(),
                    Default::default(),
                ),
            })
            .unwrap();
        assert_eq!(store.records().len(), 1);
        assert!(!store.records().is_empty());

        let ChangeOutcome::Inserted { id } = change_outcome else {
            panic!("expected inserted outcome")
        };

        let _ = store.commit(RecordChange::Delete { id }).unwrap();
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
}
