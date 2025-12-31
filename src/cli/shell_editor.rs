use crate::domain::{Editor, NewEntry};

use anyhow::{Context, Result, anyhow, bail};
use std::{fs, io::Write, path::Path, process::Command};
use tempfile::Builder;

pub struct ShellEditor;

impl ShellEditor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ShellEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor for ShellEditor {
    fn edit(&mut self, entry: &mut NewEntry) -> anyhow::Result<()> {
        let mut file = Builder::new()
            .suffix(".toml")
            .tempfile()
            .context("creating temp file for editor")?;

        let toml_text = toml::to_string_pretty(&entry).context("serializing entry to TOML")?;

        file.write_all(toml_text.as_bytes())
            .context("writing initial entry to temp file")?;
        file.flush()?;

        open_editor(file.path())?;

        let edited_text = fs::read_to_string(file.path())?;
        if edited_text == toml_text {
            println!("No changes made to the entry");
            return Ok(());
        }

        let edited_entry: NewEntry = toml::from_str(&edited_text)
            .context("parsing edited TOML (did you break the syntax?)")?;

        *entry = edited_entry;

        Ok(())
    }
}

// TODO: Accept Editor from config file
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

    let status = Command::new(cmd)
        .args(parts)
        .arg(path)
        .status()
        .context("failed to launch editor")?;

    if !status.success() {
        bail!("editor exited with {}", status);
    }

    Ok(())
}
