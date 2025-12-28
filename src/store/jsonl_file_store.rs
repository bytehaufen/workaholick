use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Write};
use std::path::PathBuf;
use std::{fs::OpenOptions, path::Path};

use anyhow::Context;

use super::EntryStore;
use crate::domain::{Entry, EntryId, EntryVersion, NewEntry};

const BUFFER_SIZE: usize = 1024 * 1024; // 1 MB
const VERSION: EntryVersion = EntryVersion(1);

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
    pub fn new(file_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let file_path: &Path = file_path.as_ref();
        let parent = file_path.parent().ok_or_else(|| {
            anyhow::anyhow!("Expected parent directory for file path: {:?}", file_path)
        })?;
        std::fs::create_dir_all(parent)?;
        let (lock_file, lock_path) = Self::create_lock_file(file_path)?;

        let entries = Self::read_file(file_path)?;
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

    fn read_from<R: BufRead>(reader: R, path: &Path) -> anyhow::Result<Vec<Entry>> {
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

    fn read_file(path: &Path) -> anyhow::Result<Vec<Entry>> {
        if !path.exists() {
            return Ok(Vec::new());
        };

        let file =
            std::fs::File::open(path).with_context(|| format!("open file {path:?} for reading"))?;

        let reader = BufReader::with_capacity(BUFFER_SIZE, file);
        Self::read_from(reader, path)
    }

    fn replace_file(&self, from: &Path, to: &Path) -> anyhow::Result<()> {
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

    fn persist(&mut self) -> anyhow::Result<()> {
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

    fn create_lock_file(file_path: &Path) -> anyhow::Result<(File, PathBuf)> {
        let path = file_path.with_added_extension("lock");
        let file = File::create_new(&path).map_err(|error| {
            if error.kind() == ErrorKind::AlreadyExists {
                anyhow::anyhow!(
                    "store is already in use (lock file exists): {path:?}\n\
                    If no other instance is running, remove the lock file."
                )
            } else {
                anyhow::anyhow!(error).context(format!("create lock file {path:?}"))
            }
        })?;

        Ok((file, path))
    }
}

impl EntryStore for JsonlFileStore {
    fn append(&mut self, entry: NewEntry) -> anyhow::Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;
        let entry = Entry::from_new_entry(self.next_id, VERSION, entry);

        let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);
        serde_json::to_writer(&mut writer, &entry)?;
        writeln!(writer)?;
        writer.flush()?;
        writer.get_ref().sync_all()?;

        self.entries.push(entry);
        self.next_id = EntryId::new(self.next_id.value() + 1);

        Ok(())
    }

    fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn delete_by_id(&mut self, id: &EntryId) -> anyhow::Result<bool> {
        if let Some(position) = self.entries.iter().position(|entry| entry.id() == id) {
            self.entries.remove(position);

            self.persist()?;
            return Ok(true);
        }

        Ok(false)
    }

    fn update(&mut self, id: EntryId, patch: NewEntry) -> anyhow::Result<()> {
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

    use crate::domain::{Command, DayType};

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
                        day_type: DayType::Office,
                    },
                ))
                .unwrap();
            store
                .append(NewEntry::new(
                    timestamp2,
                    Command::Out {
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
            match entries[0].payload() {
                Command::Log { description } => assert_eq!(*description, Some("a".into())),
                _other => panic!("expected Command::Log"),
            }
            match entries[1].payload() {
                Command::Log { description } => assert_eq!(*description, Some("c".into())),
                _other => panic!("expected Command::Log"),
            }
        };

        {
            let mut store = JsonlFileStore::new(&file_path).unwrap();
            store
                .append(NewEntry::new(
                    timestamp1,
                    Command::Log {
                        description: Some("a".to_string()),
                    },
                ))
                .unwrap();
            store
                .append(NewEntry::new(
                    timestamp2,
                    Command::Log {
                        description: Some("b".to_string()),
                    },
                ))
                .unwrap();
            store
                .append(NewEntry::new(
                    timestamp3,
                    Command::Log {
                        description: Some("c".to_string()),
                    },
                ))
                .unwrap();

            let deleted = store.delete_by_id(&EntryId::new(1)).unwrap();
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
                        day_type: DayType::Vacation,
                    },
                ))
                .unwrap();

            store
                .append(NewEntry::new(
                    timestamp2,
                    Command::Log {
                        description: Some("eating potato salad".into()),
                    },
                ))
                .unwrap();

            store
                .append(NewEntry::new(
                    timestamp3,
                    Command::Out {
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

            if let Command::Out { description } = store.entries()[1].payload() {
                assert_eq!(description.as_deref(), Some("un-wrapping presents"));
            } else {
                panic!("Wrong command type");
            }
        }
    }
}
