use std::collections::HashSet;
use std::fs::{self, File, remove_file};
use std::io::{BufRead, BufReader, BufWriter, Write};
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

pub fn write_all_atomic(file: &Path, events: &Vec<Entry>) -> anyhow::Result<()> {
    ensure_parent_exists(file)?;

    let temp_file = temp_path_for(file);
    let mut writer = OpenOptions::new().create(true).open(file)?;

    serde_json::to_writer(writer, events)?;

    replace_file(&temp_file, file)
}

fn replace_file(src: &Path, target: &Path) -> anyhow::Result<()> {
    if cfg!(windows) {
        fs::remove_file(target).with_context(|| format!("remove original file: {target:?}"))?;
    }

    fs::rename(src, target)
        .with_context(|| format!("replace log file via rename: {src:?} -> {target:?}"))?;
    Ok(())
}

pub fn read_entries(path: &Path) -> anyhow::Result<(u64, Vec<Entry>)> {
    let file = std::fs::File::open(path)?;
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

pub fn delete_by_id(path: &Path, target_id: u64) -> anyhow::Result<bool> {
    if !path.exists() {
        return Ok(false);
    }

    ensure_parent_exists(path)?;

    let temp_path = temp_path_for(path);

    let input = File::open(path).with_context(|| format!("open log file: {path:?}"))?;
    let mut output = BufWriter::new(
        File::create(&temp_path).with_context(|| format!("create tmp file {temp_path:?}"))?,
    );
    let reader = BufReader::new(input);

    let mut found = false;

    reader
        .lines()
        .enumerate()
        .try_for_each(|(index, line)| -> anyhow::Result<()> {
            let line = line.with_context(|| format!("read line {}", index + 1))?;

            // Skip empty
            if line.trim().is_empty() {
                return Ok(());
            }

            let entry: Entry = serde_json::from_str(&line)
                .with_context(|| format!("parse JSON on line {} (file {:?}", index + 1, path))?;

            if entry.id() == target_id {
                found = true;
                return Ok(());
            }

            writeln!(output, "{line}")
                .with_context(|| format!("write line {} (file {:?}", index + 1, path))?;

            Ok(())
        })?;

    output.flush()?;

    if !found {
        let _ = std::fs::remove_file(temp_path);
        return Ok(false);
    }

    Ok(true)
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
