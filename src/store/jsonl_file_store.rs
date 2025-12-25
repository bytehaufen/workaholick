use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::{fs::OpenOptions, path::Path};

use anyhow::Context;

use super::EntryStore;
use crate::domain::Entry;

const BUFFER_SIZE: usize = 1024 * 1024; // 1 MB

/// JSONL file-based `EntryStore` implementation
pub struct JsonlFileStore {
    path: PathBuf,
}

impl JsonlFileStore {
    // TODO: Check necessity
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn for_year(data_dir: &Path, year: i32) -> anyhow::Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join(format!("events-{}.jsonl", year));
        Ok(Self { path })
    }

    fn ensure_parent_exists(&self) -> anyhow::Result<()> {
        let parent = self
            .path
            .parent()
            .expect("File should have a parent directory");
        std::fs::create_dir_all(parent)?;
        Ok(())
    }

    fn replace_file(&self, from: &Path, to: &Path) -> anyhow::Result<()> {
        if cfg!(windows) && to.exists() {
            // Errors do not matter here
            let _ = fs::remove_file(to);
        }

        fs::rename(from, to)
            .with_context(|| format!("replace log file via rename: {from:?} -> {to:?}"))?;
        Ok(())
    }

    fn temp_path(&self) -> PathBuf {
        let mut path = self.path.to_path_buf();
        let file_name = path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .expect("File name should exist");
        path.set_file_name(format!("{file_name}.tmp"));
        path
    }
}

impl EntryStore for JsonlFileStore {
    fn append(&mut self, event: &Entry) -> anyhow::Result<()> {
        self.ensure_parent_exists()?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, event)?;
        writeln!(file)?;
        Ok(())
    }

    fn write_all(&mut self, events: &[Entry]) -> anyhow::Result<()> {
        self.ensure_parent_exists()?;
        let temp_file = self.temp_path();
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp_file)?;

        let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);

        for event in events {
            serde_json::to_writer(&mut writer, event)?;
            writeln!(writer)?;
        }

        writer.flush()?;
        writer.get_ref().sync_all()?;

        self.replace_file(&temp_file, &self.path)
    }

    fn read_all(&self) -> anyhow::Result<(u64, Vec<Entry>)> {
        let path = &self.path;

        if !path.exists() {
            return Ok((0, Vec::new()));
        };

        let file =
            std::fs::File::open(path).with_context(|| format!("open file {path:?} for reading"))?;

        let reader = BufReader::with_capacity(BUFFER_SIZE, file);

        let (seen, entries) = reader
            .lines()
            .enumerate()
            .filter_map(|(index, line)| match line {
                Ok(line) if line.trim().is_empty() => None,
                other => Some((index, other)),
            })
            .try_fold(
                (
                    HashSet::<u64>::with_capacity(10000),
                    Vec::<Entry>::with_capacity(10000),
                ),
                |(mut seen, mut out),
                 (index, line)|
                 -> anyhow::Result<(HashSet<u64>, Vec<Entry>)> {
                    let line = line?;
                    let entry: Entry = serde_json::from_str(&line).with_context(|| {
                        format!("parse JSON on line {} (file {:?})", index + 1, path)
                    })?;

                    if !seen.insert(entry.id()) {
                        return Err(anyhow::anyhow!(
                            "line {}: duplicate entry id {}",
                            index + 1,
                            entry.id()
                        ));
                    }

                    out.push(entry);

                    Ok((seen, out))
                },
            )?;

        let max: u64 = *seen.iter().max().unwrap_or(&0);

        Ok((max, entries))
    }

    fn delete_by_id(&mut self, target_id: u64) -> anyhow::Result<bool> {
        let (_max, mut entries) = self.read_all()?;

        if let Some(position) = entries.iter().position(|entry| entry.id() == target_id) {
            entries.remove(position);

            self.write_all(&entries)?;
            return Ok(true);
        }

        Ok(false)
    }
}
