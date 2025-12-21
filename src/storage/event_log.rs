use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::{fs::OpenOptions, path::Path};

use crate::domain::Command;

pub fn append(path: &Path, event: &Command) -> anyhow::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, event)?;
    writeln!(file)?;
    Ok(())
}

pub fn read_all(path: &Path) -> anyhow::Result<Vec<Command>> {
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

pub fn events_file(data_dir: &Path, year: i32) -> PathBuf {
    data_dir.join(format!("events-{}.jsonl", year))
}
