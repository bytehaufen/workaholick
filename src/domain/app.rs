use chrono::{DateTime, Local, NaiveDate};
use std::process::Command as ProcCommand;
use std::{fs, io::Write, path::Path};
use tempfile::NamedTempFile;

use crate::{
    cli::{self, Period},
    domain::{Command, NewEntry},
    store::EntryStore,
};

use anyhow::{Context, Result, anyhow, bail};

#[derive(Debug, Clone, Default)]
pub struct App<S: EntryStore> {
    store: S,
}

impl<S: EntryStore> App<S> {
    // TODO:
    // - Create methods: day, report
    pub fn run(store: S) -> Result<()> {
        let mut app = App { store };

        cli::run(&mut app)
    }

    pub fn write(&mut self, command: Command) -> Result<()> {
        let now = Self::now();
        let entry = NewEntry::new(now, command);

        self.store.append(entry)
    }

    pub fn delete(&mut self, id: u64) -> Result<()> {
        if !self.store.delete_by_id(&id.into())? {
            println!("ID {id} does not exist.");
        }

        Ok(())
    }

    pub fn status(&self, _period: Period) -> Result<()> {
        todo!()
    }

    pub fn day(&self, _date: NaiveDate) -> Result<()> {
        let entries = self.store.entries();

        // TODO: Implement
        println!("{entries:#?}");
        Ok(())
    }

    fn now() -> DateTime<Local> {
        Local::now()
    }

    pub fn edit(&mut self, id: u64) -> Result<()> {
        let mut entry: NewEntry = self
            .store
            .find_by_id(id.into())
            .with_context(|| format!("find ID {id}"))?
            .into();

        edit_entry_in_editor(&mut entry)?;

        self.store.update(id.into(), entry)?;

        Ok(())
    }
}

fn edit_entry_in_editor(entry: &mut NewEntry) -> Result<()> {
    let mut file = NamedTempFile::new().context("creating temp file for editor")?;

    let toml_text = toml::to_string_pretty(&entry).context("serializing entry to TOML")?;

    file.write_all(toml_text.as_bytes())
        .context("writing initial entry to temp file")?;
    file.flush()?;

    open_editor(file.path())?;

    let edited_text = fs::read_to_string(file.path())?;
    let edited_entry: NewEntry =
        toml::from_str(&edited_text).context("parsing edited TOML (did you break the syntax?)")?;

    *entry = edited_entry;

    Ok(())
}

fn open_editor(path: &Path) -> Result<()> {
    let editor = std::env::var("VISUAL")
        .ok()
        .filter(|env_var| !env_var.trim().is_empty())
        .or_else(|| std::env::var("EDITOR").ok())
        .unwrap_or_else(|| "vi".to_string());

    let mut parts =
        shlex::split(&editor).ok_or_else(|| anyhow!("Could not parse EDITOR: {editor:?}"))?;

    if parts.is_empty() {
        parts.push("vi".into());
    }

    let cmd = parts.remove(0);

    let status = ProcCommand::new(cmd)
        .args(parts)
        .arg(path)
        .status()
        .context("failed to launch editor")?;

    if !status.success() {
        bail!("editor exited with {}", status);
    }

    Ok(())
}
