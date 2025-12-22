use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::{fs::OpenOptions, path::Path};

use crate::domain::Entry;

pub fn append(file: &Path, event: &Entry) -> anyhow::Result<()> {
    ensure_parent_exists(file)?;

    let mut file = OpenOptions::new().create(true).append(true).open(file)?;
    serde_json::to_writer(&mut file, event)?;
    writeln!(file)?;
    Ok(())
}

pub fn read_all(path: &Path) -> anyhow::Result<Vec<Entry>> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    reader
        .lines()
        .enumerate()
        .filter_map(|(index, line)| match line {
            Ok(line) if line.trim().is_empty() => None,
            other => Some((index, other)),
        })
        .map(|(index, line)| {
            let line = line?;
            serde_json::from_str(&line)
                .map_err(|error| anyhow::anyhow!("line {}: {}", index + 1, error))
        })
        .collect::<anyhow::Result<Vec<_>>>()
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
