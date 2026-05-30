use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use crate::domain::record_set::{ChangeOutcome, RecordChange, RecordSet};
use crate::domain::{Record, RecordId, RecordInput};

use super::{RecordReader, RecordWriter};

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
    pub fn new(file_path: impl AsRef<Path>) -> Result<Self> {
        let file_path: &Path = file_path.as_ref();
        let parent = file_path
            .parent()
            .ok_or_else(|| anyhow!("Expected parent directory for file path: {:?}", file_path))?;
        fs::create_dir_all(parent)?;

        let (lock_file, lock_path) = Self::create_lock_file(file_path)?;

        let try_load = || {
            let records = Self::read_file(file_path)?;
            RecordSet::try_from_records(records)
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

    fn read_from<R: BufRead>(reader: R, path: &Path) -> Result<Vec<Record>> {
        reader
            .lines()
            .enumerate()
            .filter_map(|(index, line)| match line {
                Ok(line) if line.trim().is_empty() => None,
                other => Some((index, other)),
            })
            .map(|(index, line)| {
                let line = line?;
                let line_num = index + 1;

                serde_json::from_str::<Record>(&line).map_err(|e| {
                    anyhow!(
                        "parse JSON on line {} (file {:?}): {}\n  | {}",
                        line_num,
                        path,
                        e.to_string().replace(" at line 1", ""), // Remove serde's useless line info
                        line.trim()
                    )
                })
            })
            .collect()
    }

    /// Reads in the file at `path` in a by timeestamp sorted `Vec<Record>`
    fn read_file(path: &Path) -> Result<Vec<Record>> {
        if !path.exists() {
            return Ok(Vec::new());
        };

        let file = File::open(path).with_context(|| format!("open file {path:?} for reading"))?;

        let reader = BufReader::with_capacity(BUFFER_SIZE, file);
        Self::read_from(reader, path)
    }

    fn replace_file(&self, from: &Path, to: &Path) -> Result<()> {
        #[cfg(windows)]
        {
            if to.exists() {
                let _ = fs::remove_file(to);
            }
        }

        fs::rename(from, to)
            .with_context(|| format!("replace log file via rename: {from:?} -> {to:?}"))?;
        Ok(())
    }

    fn temp_file_path(path: &Path) -> PathBuf {
        path.with_added_extension("tmp")
    }

    fn persist(&mut self) -> Result<()> {
        let file_path = &self.file_path;
        let temp_file = Self::temp_file_path(file_path);

        let file = OpenOptions::new().create(true).truncate(true).write(true).open(&temp_file)?;

        let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);

        self.records_by_id.iter().try_for_each(|record| -> anyhow::Result<()> {
            serde_json::to_writer(&mut writer, record)?;
            writeln!(writer)?;
            Ok(())
        })?;

        writer.flush()?;
        writer.get_ref().sync_all()?;

        self.replace_file(&temp_file, &self.file_path)
    }

    fn create_lock_file(file_path: &Path) -> Result<(File, PathBuf)> {
        let path = file_path.with_added_extension("lock");
        let file = File::create_new(&path).map_err(|error| {
            if error.kind() == ErrorKind::AlreadyExists {
                anyhow!(
                    "store is already in use (lock file exists): {path:?}\n\
                    If no other instance is running, remove the lock file."
                )
            } else {
                anyhow!("create lock file {path:?}: {error}")
            }
        })?;

        Ok((file, path))
    }
}

impl RecordReader for JsonlFileStore {
    // TODO: Re-think over return value after refactor
    fn iter(&self) -> Box<dyn Iterator<Item = &Record> + '_> {
        Box::new(self.records_by_id.iter())
    }

    fn get(&self, id: &RecordId) -> Option<&Record> {
        self.records_by_id.get(id)
    }

    fn len(&self) -> usize {
        self.records_by_id.len()
    }

    fn is_empty(&self) -> bool {
        self.records_by_id.is_empty()
    }
}

impl RecordWriter for JsonlFileStore {
    fn next_id(&self) -> RecordId {
        self.records_by_id.next_id()
    }

    fn insert(&mut self, input: RecordInput) -> anyhow::Result<RecordId> {
        // TODO: Update later, when abstraction matches
        let id = self.records_by_id.next_id();
        let change = RecordChange::Insert { id, input };
        let outcome = self.records_by_id.apply_change(&change)?;

        if let ChangeOutcome::Inserted { id } = outcome {
            self.persist()?;

            return Ok(id);
        }

        // TODO: Should not be present after refactor
        anyhow::bail!("already catched")
    }

    fn delete(&mut self, id: &RecordId) -> anyhow::Result<Record> {
        // TODO: Update later, when abstraction matches
        let change = RecordChange::Delete { id: *id };
        let outcome = self.records_by_id.apply_change(&change)?;

        if let ChangeOutcome::Deleted { record } = outcome {
            self.persist()?;

            return Ok(record);
        }

        // TODO: Should not be present after refactor
        anyhow::bail!("already catched")
    }

    fn update(&mut self, id: &RecordId, input: RecordInput) -> Result<()> {
        // TODO: Update later, when abstraction matches
        let change = RecordChange::Update { id: *id, input };
        let outcome = self.records_by_id.apply_change(&change)?;
        if let ChangeOutcome::Updated { previous: _ } = outcome {
            self.persist()?;

            return Ok(());
        }

        // TODO: Should not be present after refactor
        anyhow::bail!("already catched")
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone, Timelike};
    use std::path::Path;

    use crate::domain::{RecordInput, WorkCommand, WorkContext, WorkRecord, WorkRecordInput};

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

    #[test]
    fn insert_persists_and_reopen_reads_records() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            store
                .insert(work_input(
                    Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap(),
                    WorkCommand::Start { context: WorkContext::Office },
                ))
                .unwrap();
            store
                .insert(work_input(
                    Local.with_ymd_and_hms(2026, 2, 3, 4, 5, 6).unwrap(),
                    WorkCommand::End {
                        title: "The main thing".into(),
                        description: Some("did some stuff".into()),
                    },
                ))
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
            store
                .insert(work_input(
                    Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap(),
                    WorkCommand::Log { title: "the a thing".into(), description: Some("a".into()) },
                ))
                .unwrap();
            store
                .insert(work_input(
                    Local.with_ymd_and_hms(2026, 2, 3, 4, 5, 6).unwrap(),
                    WorkCommand::Log { title: "the b thing".into(), description: Some("b".into()) },
                ))
                .unwrap();
            store
                .insert(work_input(
                    Local.with_ymd_and_hms(2027, 3, 4, 5, 6, 7).unwrap(),
                    WorkCommand::Log { title: "the c thing".into(), description: Some("c".into()) },
                ))
                .unwrap();

            let deleted = store.delete(&RecordId::from(1)).unwrap();
            assert_eq!(
                *work_record(&deleted).command(),
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
            store
                .insert(work_input(
                    Local.with_ymd_and_hms(2025, 12, 24, 17, 0, 0).unwrap(),
                    WorkCommand::Start { context: WorkContext::Home },
                ))
                .unwrap();
            store
                .insert(work_input(
                    Local.with_ymd_and_hms(2025, 12, 24, 14, 0, 0).unwrap(),
                    WorkCommand::Log {
                        title: "So hungry".into(),
                        description: Some("eating potato salad".into()),
                    },
                ))
                .unwrap();
            store
                .insert(work_input(
                    Local.with_ymd_and_hms(2025, 12, 24, 20, 0, 0).unwrap(),
                    WorkCommand::End {
                        title: "So excited".into(),
                        description: Some("unwrapping presents".into()),
                    },
                ))
                .unwrap();
        }

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            store
                .update(
                    &RecordId::from(1),
                    work_input(
                        Local.with_ymd_and_hms(2025, 12, 24, 18, 0, 0).unwrap(),
                        WorkCommand::End {
                            title: "So excited".into(),
                            description: Some("un-wrapping presents".into()),
                        },
                    ),
                )
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

        store
            .insert(work_input(
                Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                WorkCommand::Log { title: "the a thing".into(), description: Some("a".into()) },
            ))
            .unwrap();

        let update_err = store
            .update(
                &999.into(),
                work_input(
                    Local.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                    WorkCommand::Log { title: "the b thing".into(), description: Some("b".into()) },
                ),
            )
            .unwrap_err();
        assert_eq!(update_err.to_string(), "id 999 does not exist");

        let delete_err = store.delete(&RecordId::from(123)).unwrap_err();
        assert_eq!(delete_err.to_string(), "id 123 does not exist");
    }

    #[test]
    fn insert_after_reopen_continues_ids() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            store
                .insert(work_input(
                    Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                    WorkCommand::Log { title: "a".into(), description: None },
                ))
                .unwrap();
            store
                .insert(work_input(
                    Local.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                    WorkCommand::Log { title: "b".into(), description: None },
                ))
                .unwrap();
        }

        let mut store = JsonlFileStore::new(&file_path).unwrap();
        store
            .insert(work_input(
                Local.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                WorkCommand::Log { title: "c".into(), description: None },
            ))
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
        let ids: Vec<_> = store.iter().map(|record| record.id().value()).collect();
        assert_eq!(ids, vec![2, 1]);
    }

    #[test]
    fn get_returns_record_for_existing_id_and_none_for_missing_id() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");
        let mut store = JsonlFileStore::new(&file_path).unwrap();

        let id = store
            .insert(work_input(
                Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                WorkCommand::Log { title: "a".into(), description: Some("b".into()) },
            ))
            .unwrap();

        let record = store.get(&id).expect("record should be present");
        assert_eq!(
            *work_record(record).command(),
            WorkCommand::Log { title: "a".into(), description: Some("b".into()) }
        );
        assert!(store.get(&999.into()).is_none());
    }

    #[test]
    fn new_rejects_path_without_parent_directory() {
        let file_path = PathBuf::from("/");
        let err = JsonlFileStore::new(&file_path).unwrap_err();
        assert!(err.to_string().contains("Expected parent directory for file path"));
    }

    #[test]
    #[cfg(unix)]
    fn new_adds_context_when_opening_existing_file_fails() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        File::create(&file_path).unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o000);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let err = JsonlFileStore::new(&file_path).unwrap_err().to_string();
        assert!(err.contains("open file"));
        assert!(err.contains("for reading"));

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
            .insert(work_input(
                Local.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                WorkCommand::Log { title: "boom".into(), description: None },
            ))
            .unwrap_err()
            .to_string();

        assert!(err.contains("replace log file via rename"));
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
        assert_eq!(store.records_by_id.get(&0.into()).unwrap().id().value(), 0);
    }

    #[test]
    fn read_file_reports_line_number_on_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "{{ this is not json }}").unwrap();

        let err = JsonlFileStore::new(&file_path).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("parse JSON on line 1"));
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
        assert_eq!(error.to_string(), "duplicate id 0 in records");
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
        assert!(msg.contains("create lock file"));
        assert!(msg.contains("Permission denied"));
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
        let ids: Vec<_> = store.iter().map(|record| record.id().value()).collect();
        assert_eq!(ids, vec![3, 2, 0, 1]);
    }

    #[test]
    fn len_and_is_empty_reflect_store_state() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.jsonl");

        let mut store = JsonlFileStore::new(&file_path).unwrap();
        assert_eq!(store.len(), 0);
        assert!(store.is_empty());

        let id = store
            .insert(work_input(
                Local.with_ymd_and_hms(2026, 3, 15, 5, 15, 0).unwrap(),
                Default::default(),
            ))
            .unwrap();
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());

        store.delete(&id).unwrap();
        assert_eq!(store.len(), 0);
        assert!(store.is_empty());
    }
}
