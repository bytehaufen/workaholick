use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::{fs::OpenOptions, path::Path};

use anyhow::Context;

use crate::domain::Entry;

pub fn append(file: &Path, event: &Entry) -> anyhow::Result<()> {
    ensure_parent_exists(file)?;

    let mut file = OpenOptions::new().create(true).append(true).open(file)?;
    serde_json::to_writer(&mut file, event)?;
    writeln!(file)?;
    Ok(())
}

pub fn write_all_atomic(file: &Path, events: &[Entry]) -> anyhow::Result<()> {
    ensure_parent_exists(file)?;

    let temp_file = temp_path_for(file);
    let writer = OpenOptions::new()
        .create(true)
        .truncate(true)
        .open(&temp_file)?;

    serde_json::to_writer(writer, events)?;

    replace_file(&temp_file, file)
}

fn replace_file(from: &Path, to: &Path) -> anyhow::Result<()> {
    if cfg!(windows) {
        fs::remove_file(to).with_context(|| format!("remove original file: {to:?}"))?;
    }

    fs::rename(from, to)
        .with_context(|| format!("replace log file via rename: {from:?} -> {to:?}"))?;
    Ok(())
}

pub fn read_entries(path: &Path) -> anyhow::Result<(u64, Vec<Entry>)> {
    if !std::fs::exists(path).with_context(|| format!("check file {path:?} exists"))? {
        return Ok((0, Vec::new()));
    };
    let file =
        std::fs::File::open(path).with_context(|| format!("open file {path:?} for reading"))?;
    let reader = BufReader::new(file);

    let (seen, entries) = reader
        .lines()
        .enumerate()
        .filter_map(|(index, line)| match line {
            Ok(line) if line.trim().is_empty() => None,
            other => Some((index, other)),
        })
        .try_fold(
            (HashSet::<u64>::new(), Vec::<Entry>::new()),
            |(mut seen, mut out), (index, line)| -> anyhow::Result<(HashSet<u64>, Vec<Entry>)> {
                let line = line?;
                let entry: Entry = serde_json::from_str(&line).with_context(|| {
                    format!("parse JSON on line {} (file {:?}", index + 1, path)
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

pub fn delete_by_id(file: &Path, target_id: u64) -> anyhow::Result<bool> {
    let (_max, mut entries) = read_entries(file)?;

    if let Some(position) = entries.iter().position(|entry| entry.id() == target_id) {
        entries.remove(position);

        write_all_atomic(file, &entries)?;
        return Ok(true);
    }

    Ok(false)
}

pub fn events_file(data_dir: &Path, year: i32) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(data_dir)?;

    Ok(data_dir.join(format!("events-{}.jsonl", year)))
}

fn ensure_parent_exists(file: &Path) -> anyhow::Result<()> {
    let parent = file.parent().expect("File should have a parent directory");
    std::fs::create_dir_all(parent)?;
    Ok(())
}

fn temp_path_for(path: &Path) -> PathBuf {
    let mut path = path.to_path_buf();
    let file_name = path
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .expect("File name should exist");
    path.set_file_name(format!("{file_name}.tmp"));
    path
}
