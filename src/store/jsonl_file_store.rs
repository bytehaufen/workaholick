use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::{fs::OpenOptions, path::Path};

use anyhow::Context;
use chrono::Datelike;

use super::EntryStore;
use crate::domain::Entry;

const BUFFER_SIZE: usize = 1024 * 1024; // 1 MB

/// JSONL file-based `EntryStore` implementation
pub struct JsonlFileStore {
    file_path: PathBuf,
    lock_file: File,
    lock_path: PathBuf,
    entries: Vec<Entry>,
}

impl Drop for JsonlFileStore {
    fn drop(&mut self) {
        // Lock-file is auto dropped, so we only remove it
        let _ = fs::remove_file(&self.lock_path);
    }
}

impl JsonlFileStore {
    pub fn new(file_path: &str) -> anyhow::Result<Self> {
        std::fs::create_dir_all(file_path)?;
        let file_path: &Path = file_path.as_ref();
        let (lock_file, lock_path) = Self::create_lock_file(file_path)?;

        let entries = Self::read_file(file_path)?;

        Ok(Self {
            file_path: file_path.into(),
            lock_file,
            lock_path,
            entries,
        })
    }

    fn current_year() -> i32 {
        chrono::Local::now().year()
    }

    fn read_file(path: &Path) -> anyhow::Result<Vec<Entry>> {
        if !path.exists() {
            return Ok(Vec::new());
        };

        let file =
            std::fs::File::open(path).with_context(|| format!("open file {path:?} for reading"))?;

        let reader = BufReader::with_capacity(BUFFER_SIZE, file);

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

    fn write_all(&mut self, entries: &[Entry]) -> anyhow::Result<()> {
        let file_path = &self.file_path;
        let temp_file = Self::temp_file_path(file_path);

        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp_file)?;

        let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);

        for entry in entries {
            serde_json::to_writer(&mut writer, entry)?;
            writeln!(writer)?;
        }

        writer.flush()?;
        writer.get_ref().sync_all()?;

        self.replace_file(&temp_file, &self.file_path)
    }

    fn create_lock_file(data_dir: &Path) -> anyhow::Result<(File, PathBuf)> {
        let path = data_dir.join("entries.lock");
        let file = File::create_new(&path)?;

        Ok((file, path))
    }
}

impl EntryStore for JsonlFileStore {
    fn append(&mut self, entry: &Entry) -> anyhow::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;
        serde_json::to_writer(&mut file, entry)?;
        writeln!(file)?;

        Ok(())
    }

    fn read_all(&self) -> anyhow::Result<Vec<Entry>> {
        let path = &self.file_path;

        Self::read_file(path)
    }

    fn delete_by_id(&mut self, target_id: u64) -> anyhow::Result<bool> {
        let mut entries = self.read_all()?;

        if let Some(position) = entries.iter().position(|entry| entry.id() == target_id) {
            entries.remove(position);

            self.write_all(&entries)?;
            return Ok(true);
        }

        Ok(false)
    }

    fn update(&mut self, id: u64, patch: &Entry) -> anyhow::Result<()> {
        todo!()
    }
}
