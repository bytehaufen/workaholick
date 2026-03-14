use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use indexmap::IndexMap;

use crate::domain::{Entry, EntryId, EntryPatch};

use super::{EntryReader, EntryWriter};

const BUFFER_SIZE: usize = 1024 * 1024; // 1 MB
const VERSION: u8 = 1;

/// JSONL file-based `EntryStore` implementation
#[derive(Debug)]
pub struct JsonlFileStore {
    file_path: PathBuf,
    _lock_file: File,
    lock_path: PathBuf,
    entries: IndexMap<EntryId, Entry>,
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
            let entries = Self::read_file(file_path)?;
            Self::normalize_entries(entries)
        };

        let entries = match try_load() {
            Ok(entries) => entries,
            Err(error) => {
                drop(lock_file);
                let _ = fs::remove_file(&lock_path);
                return Err(error);
            }
        };

        Ok(Self { file_path: file_path.into(), _lock_file: lock_file, lock_path, entries })
    }

    fn read_from<R: BufRead>(reader: R, path: &Path) -> Result<Vec<Entry>> {
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

                serde_json::from_str::<Entry>(&line).map_err(|e| {
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

    /// Reads in the file at `path` in a by timeestamp sorted `Vec<Entry>`
    fn read_file(path: &Path) -> Result<Vec<Entry>> {
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

        self.entries.iter().try_for_each(|(_, entry)| -> anyhow::Result<()> {
            serde_json::to_writer(&mut writer, entry)?;
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

    fn insert_sorted(&mut self, id: EntryId, entry: Entry) {
        self.entries
            .insert_sorted_by(id, entry, |_i1, e1, _i2, e2| e1.sort_key().cmp(&e2.sort_key()));
    }

    fn normalize_entries(
        entries: Vec<Entry>,
    ) -> std::result::Result<IndexMap<EntryId, Entry>, anyhow::Error> {
        let len = entries.len();
        let mut entries =
            entries.into_iter().try_fold(IndexMap::with_capacity(len), |mut entries, entry| {
                let id = *entry.id();
                if entries.insert(id, entry).is_some() {
                    anyhow::bail!("duplicate id {id} in entries");
                }

                Ok(entries)
            })?;

        entries.sort_by_key(|_, e| e.sort_key());

        Ok(entries)
    }
}

impl EntryReader for JsonlFileStore {
    fn iter(&self) -> Box<dyn Iterator<Item = &Entry> + '_> {
        Box::new(self.entries.values())
    }

    fn get(&self, id: &EntryId) -> Option<&Entry> {
        self.entries.get(id)
    }
}

impl EntryWriter for JsonlFileStore {
    fn next_id(&self) -> EntryId {
        self.entries.keys().max().map(|id| id.incremented()).unwrap_or_default()
    }

    fn insert(&mut self, new_entry: EntryPatch) -> anyhow::Result<EntryId> {
        let id = self.next_id();

        let new_entry = Entry::from_patch(id, VERSION.into(), new_entry);

        self.insert_sorted(id, new_entry);

        self.persist()?;

        Ok(id)
    }

    fn remove(&mut self, id: &EntryId) -> anyhow::Result<Entry> {
        // We do a slow shift remove, to do not mess the order in the stored file
        let entry =
            self.entries.shift_remove(id).with_context(|| format!("id {id} does not exist"))?;

        self.persist()?;

        Ok(entry)
    }

    fn update(&mut self, id: &EntryId, patch: EntryPatch) -> Result<()> {
        let entry = self.entries.get_mut(id).with_context(|| format!("id {id} does not exist"))?;

        let updated_entry = Entry::from_patch(*id, *entry.version(), patch);
        self.entries.shift_remove(id);
        self.insert_sorted(*id, updated_entry);

        self.persist()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone, Timelike};

    use crate::domain::{ActivityKind, Command, WorkContext};

    use super::*;

    #[test]
    fn insert_persists_and_reopen_reads_entries() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        let timestamp1 = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let timestamp2 = Local.with_ymd_and_hms(2026, 2, 3, 4, 5, 6).unwrap();

        let assert_store = |store: &JsonlFileStore| {
            assert_eq!(store.entries.len(), 2);
            assert_eq!(store.entries[0].id().value(), 0);
            assert_eq!(store.entries[1].id().value(), 1);
        };

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();

            store
                .insert(EntryPatch::new(
                    timestamp1,
                    Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } },
                ))
                .unwrap();
            store
                .insert(EntryPatch::new(
                    timestamp2,
                    Command::End {
                        title: "The main thing".into(),
                        description: Some("did some stuff".to_string()),
                    },
                ))
                .unwrap();

            assert_store(&store);
        }
        {
            let store = JsonlFileStore::new(&file_path).unwrap();

            assert_store(&store);
        }
    }

    #[test]
    fn delete_by_id_persists() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        let timestamp1 = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let timestamp2 = Local.with_ymd_and_hms(2026, 2, 3, 4, 5, 6).unwrap();
        let timestamp3 = Local.with_ymd_and_hms(2027, 3, 4, 5, 6, 7).unwrap();

        let assert_store = |store: &JsonlFileStore| {
            let entries = &store.entries;
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].id().value(), 0);
            assert_eq!(entries[1].id().value(), 2);

            assert_eq!(
                *entries[0].command(),
                Command::Log { title: "the a thing".into(), description: Some("a".into()) }
            );
            assert_eq!(
                *entries[1].command(),
                Command::Log { title: "the c thing".into(), description: Some("c".into()) }
            );
        };

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            store
                .insert(EntryPatch::new(
                    timestamp1,
                    Command::Log {
                        title: "the a thing".into(),
                        description: Some("a".to_string()),
                    },
                ))
                .unwrap();
            store
                .insert(EntryPatch::new(
                    timestamp2,
                    Command::Log {
                        title: "the b thing".into(),
                        description: Some("b".to_string()),
                    },
                ))
                .unwrap();
            store
                .insert(EntryPatch::new(
                    timestamp3,
                    Command::Log {
                        title: "the c thing".into(),
                        description: Some("c".to_string()),
                    },
                ))
                .unwrap();

            let deleted = store.remove(&EntryId::from(1)).unwrap();
            assert_eq!(
                *deleted.command(),
                Command::Log { title: "the b thing".into(), description: Some("b".into()) }
            );

            assert_store(&store);
        }
        {
            let store = JsonlFileStore::new(&file_path).unwrap();
            assert_store(&store);
        }
    }

    #[test]
    fn lock_file_prevents_second_instance() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        let _store1 = JsonlFileStore::new(&file_path).unwrap();
        let err = JsonlFileStore::new(&file_path).unwrap_err();

        assert!(err.to_string().contains("lock file exists"));
    }

    #[test]
    fn update_entry() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        let assert_store = |store: &JsonlFileStore, id1, id2, id3| {
            assert_eq!(store.entries.len(), 3);
            assert_eq!(store.entries[0].id().value(), id1);
            assert_eq!(store.entries[1].id().value(), id2);
            assert_eq!(store.entries[2].id().value(), id3);
        };

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            let timestamp1 = Local.with_ymd_and_hms(2025, 12, 24, 17, 0, 0).unwrap();
            let timestamp2 = Local.with_ymd_and_hms(2025, 12, 24, 14, 0, 0).unwrap();
            let timestamp3 = Local.with_ymd_and_hms(2025, 12, 24, 20, 0, 0).unwrap();

            store
                .insert(EntryPatch::new(
                    timestamp1,
                    Command::Start { kind: ActivityKind::Vacation },
                ))
                .unwrap();

            store
                .insert(EntryPatch::new(
                    timestamp2,
                    Command::Log {
                        title: "So hungry".into(),
                        description: Some("eating potato salad".into()),
                    },
                ))
                .unwrap();

            store
                .insert(EntryPatch::new(
                    timestamp3,
                    Command::End {
                        title: "So excited".into(),
                        description: Some("unwrapping presents".into()),
                    },
                ))
                .unwrap();

            assert_store(&store, 1, 0, 2);
        }
        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            let updated = EntryPatch::new(
                Local.with_ymd_and_hms(2025, 12, 24, 18, 0, 0).unwrap(),
                Command::End {
                    title: "So excited".into(),
                    description: Some("un-wrapping presents".into()),
                },
            );
            store.update(&EntryId::from(1), updated).unwrap();

            assert_store(&store, 0, 1, 2);
        }
        {
            let store = JsonlFileStore::new(&file_path).unwrap();

            assert_store(&store, 0, 1, 2);
            assert_eq!(store.entries[1].timestamp().hour(), 18);

            assert_eq!(
                *store.entries[1].command(),
                Command::End {
                    title: "So excited".into(),
                    description: Some("un-wrapping presents".into())
                }
            );
        }
    }

    #[test]
    fn update_missing_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        let mut store = JsonlFileStore::new(&file_path).unwrap();

        store
            .insert(EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                Command::Log { title: "the a thing".into(), description: Some("a".into()) },
            ))
            .unwrap();

        let patch = EntryPatch::new(
            Local.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
            Command::Log { title: "the b thing".into(), description: Some("b".into()) },
        );

        let err = store.update(&999.into(), patch).unwrap_err();
        assert_eq!(err.to_string(), "id 999 does not exist");
    }

    #[test]
    fn delete_missing_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        let mut store = JsonlFileStore::new(&file_path).unwrap();

        store
            .insert(EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                Command::Log { title: "the a thing".into(), description: Some("a".into()) },
            ))
            .unwrap();

        let error = store.remove(&EntryId::from(123)).unwrap_err();

        assert_eq!(error.to_string(), "id 123 does not exist");

        assert_eq!(store.entries.len(), 1);
        assert_eq!(store.entries[0].id().value(), 0);
    }

    #[test]
    fn insert_after_reopen_continues_ids() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            store
                .insert(EntryPatch::new(
                    Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                    Command::Log { title: "the a thing".into(), description: Some("a".into()) },
                ))
                .unwrap();
            store
                .insert(EntryPatch::new(
                    Local.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                    Command::Log { title: "the b thing".into(), description: Some("b".into()) },
                ))
                .unwrap();
        }

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            store
                .insert(EntryPatch::new(
                    Local.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                    Command::Log { title: "the c thing".into(), description: Some("c".into()) },
                ))
                .unwrap();

            assert_eq!(store.entries.len(), 3);
            assert_eq!(store.entries[0].id().value(), 0);
            assert_eq!(store.entries[1].id().value(), 1);
            assert_eq!(store.entries[2].id().value(), 2);
        }
    }

    #[test]
    fn iter_returns_entries_sorted_by_timestamp_when_inserted() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");
        let mut store = JsonlFileStore::new(&file_path).unwrap();

        store
            .insert(EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                Command::Log { title: "late".into(), description: None },
            ))
            .unwrap();
        store
            .insert(EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                Command::Log { title: "early".into(), description: None },
            ))
            .unwrap();
        store
            .insert(EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                Command::Log { title: "mid".into(), description: None },
            ))
            .unwrap();

        let hours: Vec<u32> = store.iter().map(|entry| entry.timestamp().hour()).collect();
        assert_eq!(hours, vec![10, 11, 12]);
    }

    #[test]
    fn get_returns_entry_for_existing_id_and_none_for_missing_id() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");
        let mut store = JsonlFileStore::new(&file_path).unwrap();

        let id = store
            .insert(EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                Command::Log { title: "a".into(), description: Some("b".into()) },
            ))
            .unwrap();

        let entry = store.get(&id).expect("entry should be present");
        assert_eq!(
            *entry.command(),
            Command::Log { title: "a".into(), description: Some("b".into()) }
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
        let file_path = dir.path().join("entries.jsonl");

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
        let file_path = dir.path().join("entries.jsonl");

        let mut store = JsonlFileStore::new(&file_path).unwrap();
        std::fs::create_dir(&file_path).unwrap();

        let err = store
            .insert(EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                Command::Log { title: "boom".into(), description: None },
            ))
            .unwrap_err()
            .to_string();

        assert!(err.contains("replace log file via rename"));
    }

    #[test]
    fn lock_file_is_removed_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        {
            let _store = JsonlFileStore::new(&file_path).unwrap();
        }

        // Should succeed because Drop removed the lock
        let _store2 = JsonlFileStore::new(&file_path).unwrap();
    }

    #[test]
    fn lock_file_is_removed_in_new_if_err() {
        let dir = tempfile::tempdir().unwrap();
        let store_file_path = dir.path().join("entries.jsonl");
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
        let file_path = dir.path().join("entries.jsonl");

        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let entry = Entry::from_patch(
            EntryId::from(0),
            super::VERSION.into(),
            EntryPatch::new(
                timestamp,
                Command::Log { title: "the a thing".into(), description: Some("a".into()) },
            ),
        );

        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file).unwrap();
        serde_json::to_writer(&mut file, &entry).unwrap();
        writeln!(file).unwrap();
        writeln!(file).unwrap();

        let store = JsonlFileStore::new(&file_path).unwrap();
        assert_eq!(store.entries.len(), 1);
        assert_eq!(store.entries[0].id().value(), 0);
    }

    #[test]
    fn read_file_reports_line_number_on_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "{{ this is not json }}").unwrap();

        let err = JsonlFileStore::new(&file_path).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("parse JSON on line 1"));
    }

    #[test]
    fn new_with_duplicate_ids_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let store_file_path = dir.path().join("entries.jsonl");

        let mut file = File::create(&store_file_path).unwrap();
        file.write_all(br#"{"id":0,"version":1,"timestamp":"2026-01-05 04:30:26 +0100","command":{"start":{"kind":{"work":{"context":"home"}}}}}
                                {"id":0,"version":1,"timestamp":"2026-01-05 16:45:03 +0100","command":{"end":{"title":"foo","description":null}}}"#).unwrap();
        file.flush().unwrap();

        let error = JsonlFileStore::new(&store_file_path).unwrap_err();
        assert_eq!(error.to_string(), "duplicate id 0 in entries");
    }

    #[test]
    #[cfg(unix)]
    fn create_lock_file_with_permission_error() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();

        // Create the parent directory but make it read-only
        let lock_dir = dir.path().join("readonly");
        std::fs::create_dir(&lock_dir).unwrap();
        let mut perms = std::fs::metadata(&lock_dir).unwrap().permissions();
        perms.set_mode(0o444); // read-only
        std::fs::set_permissions(&lock_dir, perms).unwrap();

        let store_in_readonly = lock_dir.join("entries.jsonl");
        let err = JsonlFileStore::new(&store_in_readonly).unwrap_err();

        // Restore permissions
        let mut perms = std::fs::metadata(&lock_dir).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&lock_dir, perms).unwrap();

        let msg = err.to_string();
        assert!(msg.contains("create lock file"));
        assert!(msg.contains("Permission denied"));
    }

    #[test]
    fn iter_returns_entries_sorted_by_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let store_file_path = dir.path().join("entries.jsonl");

        let mut file = File::create(&store_file_path).unwrap();
        file.write_all(br#"{"id":1,"version":1,"timestamp":"2026-01-05 04:30:26 +0100","command":{"start":{"kind":{"work":{"context":"home"}}}}}
                                {"id":2,"version":1,"timestamp":"2026-01-05 03:45:03 +0100","command":{"end":{"title":"foo","description":null}}}"#).unwrap();
        file.flush().unwrap();

        let store = JsonlFileStore::new(&store_file_path).unwrap();

        dbg!(&store.entries);

        let mut iter = store.iter();

        assert_eq!(*iter.next().unwrap().id(), EntryId::from(2));
        assert_eq!(*iter.next().unwrap().id(), EntryId::from(1));
        assert!(iter.next().is_none());
    }

    #[test]
    fn iter_returns_entries_sorted_by_id_when_timestamps_are_eq() {
        let dir = tempfile::tempdir().unwrap();
        let store_file_path = dir.path().join("entries.jsonl");

        let mut file = File::create(&store_file_path).unwrap();
        file.write_all(br#"{"id":2,"version":1,"timestamp":"2026-01-05 04:30:26 +0100","command":{"start":{"kind":{"work":{"context":"home"}}}}}
                                {"id":1,"version":1,"timestamp":"2026-01-05 04:30:26 +0100","command":{"end":{"title":"foo","description":null}}}"#).unwrap();
        file.flush().unwrap();

        let store = JsonlFileStore::new(&store_file_path).unwrap();

        let mut iter = store.iter();

        assert_eq!(*iter.next().unwrap().id(), EntryId::from(1));
        assert_eq!(*iter.next().unwrap().id(), EntryId::from(2));
        assert!(iter.next().is_none());
    }

    #[test]
    fn new_with_unsorted_timestamps_normalizes_iteration_order() {
        let dir = tempfile::tempdir().unwrap();
        let store_file_path = dir.path().join("entries.jsonl");

        let mut file = File::create(&store_file_path).unwrap();
        file.write_all(br#"{"id":1,"version":1,"timestamp":"2026-01-04 04:30:26 +0100","command":{"start":{"kind":{"work":{"context":"home"}}}}}
                                {"id":0,"version":1,"timestamp":"2026-01-04 04:30:26 +0100","command":{"start":{"kind":{"work":{"context":"home"}}}}}
                                {"id":2,"version":1,"timestamp":"2026-01-03 04:30:26 +0100","command":{"start":{"kind":{"work":{"context":"home"}}}}}
                                {"id":3,"version":1,"timestamp":"2026-01-02 16:45:03 +0100","command":{"start":{"kind":{"work":{"context":"home"}}}}}"#).unwrap();
        file.flush().unwrap();

        let store = JsonlFileStore::new(&store_file_path).unwrap();

        let ids: Vec<_> = store.iter().map(|entry| entry.id().value()).collect();
        assert_eq!(ids, vec![3, 2, 0, 1]);
    }
}
