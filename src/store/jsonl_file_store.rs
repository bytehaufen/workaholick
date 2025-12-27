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

    fn update(&mut self, _id: EntryId, _patch: NewEntry) -> anyhow::Result<()> {
        todo!()
    }

    fn find_by_id(&self, id: EntryId) -> Option<&Entry> {
        self.entries.iter().find(|entry| *entry.id() == id)
    }
}
