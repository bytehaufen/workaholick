use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use crate::EntryStore;
use crate::domain::{Entry, EntryId, NewEntry};

const BUFFER_SIZE: usize = 1024 * 1024; // 1 MB
const VERSION: u8 = 1;

/// JSONL file-based `EntryStore` implementation
#[derive(Debug)]
pub struct JsonlFileStore {
    file_path: PathBuf,
    _lock_file: File,
    lock_path: PathBuf,
    entries: Vec<Entry>,
    next_id: EntryId,
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

        let entries = match Self::read_file(file_path) {
            Ok(entries) => entries,
            Err(error) => {
                let _ = fs::remove_file(&lock_path);
                return Err(error);
            }
        };

        let next_id = if let Some(max) = entries.iter().map(|entry| entry.id()).max() {
            max.value() + 1
        } else {
            0
        }
        .into();

        Ok(Self {
            file_path: file_path.into(),
            _lock_file: lock_file,
            lock_path,
            entries,
            next_id,
        })
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
                let entry: Entry = serde_json::from_str(&line).with_context(|| {
                    format!("parse JSON on line {} (file {:?})", index + 1, path)
                })?;

                Ok(entry)
            })
            .collect()
    }

    fn read_file(path: &Path) -> Result<Vec<Entry>> {
        if !path.exists() {
            return Ok(Vec::new());
        };

        let file = File::open(path).with_context(|| format!("open file {path:?} for reading"))?;

        let reader = BufReader::with_capacity(BUFFER_SIZE, file);
        Self::read_from(reader, path)
    }

    fn replace_file(&self, from: &Path, to: &Path) -> Result<()> {
        if cfg!(windows) && to.exists() {
            let _ = fs::remove_file(to);
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

        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp_file)?;

        let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);

        for entry in &self.entries {
            serde_json::to_writer(&mut writer, entry)?;
            writeln!(writer)?;
        }

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
                anyhow!(error).context(format!("create lock file {path:?}"))
            }
        })?;

        Ok((file, path))
    }
}

impl EntryStore for JsonlFileStore {
    fn append(&mut self, entry: NewEntry) -> Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;
        let entry = Entry::from_new_entry(self.next_id, VERSION.into(), entry);

        let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);
        serde_json::to_writer(&mut writer, &entry)?;
        writeln!(writer)?;
        writer.flush()?;
        writer.get_ref().sync_all()?;

        self.entries.push(entry);
        self.next_id = EntryId::from(self.next_id.value() + 1);

        Ok(())
    }

    fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn delete_by_id(&mut self, id: &EntryId) -> Result<bool> {
        if let Some(position) = self.entries.iter().position(|entry| entry.id() == id) {
            self.entries.remove(position);

            self.persist()?;
            return Ok(true);
        }

        Ok(false)
    }

    fn update(&mut self, id: EntryId, patch: NewEntry) -> Result<()> {
        let position = self
            .entries
            .iter_mut()
            .position(|entry| *entry.id() == id)
            .with_context(|| format!("cannot find entry with ID {id:?}"))?;

        let existing = &self.entries[position];
        let updated = Entry::from_new_entry(id, *existing.version(), patch);

        self.entries[position] = updated;
        self.persist()?;

        Ok(())
    }

    fn find_by_id(&self, id: EntryId) -> Option<&Entry> {
        self.entries.iter().find(|entry| *entry.id() == id)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone, Timelike};

    use crate::domain::{Command, TimeKind};

    use super::*;

    #[test]
    fn append_persists_and_reopen_reads_entries() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        let timestamp1 = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let timestamp2 = Local.with_ymd_and_hms(2026, 2, 3, 4, 5, 6).unwrap();

        let assert_store = |store: &JsonlFileStore| {
            assert_eq!(store.entries().len(), 2);
            assert_eq!(store.entries()[0].id().value(), 0);
            assert_eq!(store.entries()[1].id().value(), 1);
        };

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();

            store
                .append(NewEntry::new(
                    timestamp1,
                    Command::In {
                        time_kind: TimeKind::Office,
                    },
                ))
                .unwrap();
            store
                .append(NewEntry::new(
                    timestamp2,
                    Command::Out {
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
            let entries = store.entries();
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].id().value(), 0);
            assert_eq!(entries[1].id().value(), 2);
            match entries[0].command() {
                Command::Log {
                    title: _,
                    description,
                } => assert_eq!(description.as_deref(), Some("a")),
                _other => panic!("expected Command::Log"),
            }
            match entries[1].command() {
                Command::Log {
                    title: _,
                    description,
                } => assert_eq!(description.as_deref(), Some("c")),
                _other => panic!("expected Command::Log"),
            }
        };

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            store
                .append(NewEntry::new(
                    timestamp1,
                    Command::Log {
                        title: "the a thing".into(),
                        description: Some("a".to_string()),
                    },
                ))
                .unwrap();
            store
                .append(NewEntry::new(
                    timestamp2,
                    Command::Log {
                        title: "the b thing".into(),
                        description: Some("b".to_string()),
                    },
                ))
                .unwrap();
            store
                .append(NewEntry::new(
                    timestamp3,
                    Command::Log {
                        title: "the c thing".into(),
                        description: Some("c".to_string()),
                    },
                ))
                .unwrap();

            let deleted = store.delete_by_id(&EntryId::from(1)).unwrap();
            assert!(deleted);
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

        assert!(
            err.to_string().contains("lock file exists"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn update_entry() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        let assert_store = |store: &JsonlFileStore| {
            assert_eq!(store.entries().len(), 3);
            assert_eq!(store.entries()[0].id().value(), 0);
            assert_eq!(store.entries()[1].id().value(), 1);
            assert_eq!(store.entries()[2].id().value(), 2);
        };

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            let timestamp1 = Local.with_ymd_and_hms(2025, 12, 24, 17, 0, 0).unwrap();
            let timestamp2 = Local.with_ymd_and_hms(2025, 12, 24, 14, 0, 0).unwrap();
            let timestamp3 = Local.with_ymd_and_hms(2025, 12, 24, 20, 0, 0).unwrap();

            store
                .append(NewEntry::new(
                    timestamp1,
                    Command::In {
                        time_kind: TimeKind::Vacation,
                    },
                ))
                .unwrap();

            store
                .append(NewEntry::new(
                    timestamp2,
                    Command::Log {
                        title: "So hungry".into(),
                        description: Some("eating potato salad".into()),
                    },
                ))
                .unwrap();

            store
                .append(NewEntry::new(
                    timestamp3,
                    Command::Out {
                        title: "So excited".into(),
                        description: Some("unwrapping presents".into()),
                    },
                ))
                .unwrap();

            assert_store(&store);
        }
        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            let updated = NewEntry::new(
                Local.with_ymd_and_hms(2025, 12, 24, 18, 0, 0).unwrap(),
                Command::Out {
                    title: "So excited".into(),
                    description: Some("un-wrapping presents".into()),
                },
            );
            store.update(1.into(), updated).unwrap();

            assert_store(&store);
        }
        {
            let store = JsonlFileStore::new(&file_path).unwrap();

            assert_store(&store);
            assert_eq!(store.entries()[1].timestamp().hour(), 18);

            if let Command::Out {
                title: _,
                description,
            } = store.entries()[1].command()
            {
                assert_eq!(description.as_deref(), Some("un-wrapping presents"));
            } else {
                panic!("Wrong command type");
            }
        }
    }

    #[test]
    fn update_missing_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        let mut store = JsonlFileStore::new(&file_path).unwrap();

        store
            .append(NewEntry::new(
                Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                Command::Log {
                    title: "the a thing".into(),
                    description: Some("a".into()),
                },
            ))
            .unwrap();

        let patch = NewEntry::new(
            Local.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
            Command::Log {
                title: "the b thing".into(),
                description: Some("b".into()),
            },
        );

        let err = store.update(999.into(), patch).unwrap_err();
        assert!(
            err.to_string().contains("cannot find entry with ID"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn delete_missing_id_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        let mut store = JsonlFileStore::new(&file_path).unwrap();

        store
            .append(NewEntry::new(
                Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                Command::Log {
                    title: "the a thing".into(),
                    description: Some("a".into()),
                },
            ))
            .unwrap();

        let deleted = store.delete_by_id(&EntryId::from(123)).unwrap();
        assert!(!deleted);
        assert_eq!(store.entries().len(), 1);
        assert_eq!(store.entries()[0].id().value(), 0);
    }

    #[test]
    fn append_after_reopen_continues_ids() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            store
                .append(NewEntry::new(
                    Local.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                    Command::Log {
                        title: "the a thing".into(),
                        description: Some("a".into()),
                    },
                ))
                .unwrap();
            store
                .append(NewEntry::new(
                    Local.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                    Command::Log {
                        title: "the b thing".into(),
                        description: Some("b".into()),
                    },
                ))
                .unwrap();
        }

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            store
                .append(NewEntry::new(
                    Local.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
                    Command::Log {
                        title: "the c thing".into(),
                        description: Some("c".into()),
                    },
                ))
                .unwrap();

            assert_eq!(store.entries().len(), 3);
            assert_eq!(store.entries()[0].id().value(), 0);
            assert_eq!(store.entries()[1].id().value(), 1);
            assert_eq!(store.entries()[2].id().value(), 2);
        }
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
            assert!(
                fs::exists(&lock_file_path).unwrap(),
                "Lock file does not exist"
            );
        }

        let mut store_file = fs::File::create_new(&store_file_path).unwrap();
        store_file
            .write_all("not parsable data".as_bytes())
            .unwrap();

        let _ = JsonlFileStore::new(&store_file_path).unwrap_err();

        assert!(!fs::exists(&lock_file_path).unwrap(), "Lock file exists");
    }

    #[test]
    fn read_file_ignores_blank_lines() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let entry = Entry::from_new_entry(
            EntryId::from(0),
            super::VERSION.into(),
            NewEntry::new(
                timestamp,
                Command::Log {
                    title: "the a thing".into(),
                    description: Some("a".into()),
                },
            ),
        );

        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file).unwrap();
        serde_json::to_writer(&mut file, &entry).unwrap();
        writeln!(file).unwrap();
        writeln!(file).unwrap();

        let store = JsonlFileStore::new(&file_path).unwrap();
        assert_eq!(store.entries().len(), 1);
        assert_eq!(store.entries()[0].id().value(), 0);
    }

    #[test]
    fn read_file_reports_line_number_on_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("entries.jsonl");

        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "{{ this is not json }}").unwrap();

        let err = JsonlFileStore::new(&file_path).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("parse JSON on line 1"),
            "unexpected error: {msg}"
        );
    }
}
