use anyhow::{Context, Result};
use ron::ser::PrettyConfig;
use strum::IntoEnumIterator;

use crate::domain::{Command, NewEntry};

pub fn render_for_edit(entry: &NewEntry) -> Result<String> {
    let pretty_config = PrettyConfig::new();

    let ron_text = ron::ser::to_string_pretty(&entry, pretty_config.clone())
        .context("serializing entry to RON")?;

    let header = "// Edit the entry below.\n\
         // Save and close the editor to apply changes.\n\
         // Keep the format valid.\n\
         //\n\
         // Available Commands are:\n\n"
        .to_string();

    let templates: String = Command::iter()
        .map(|command| ron::ser::to_string(&command).context("serializing command template"))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .map(|line| format!("// {line}"))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!("{header}{templates}\n{ron_text}"))
}

pub fn parse_edited(text: &str) -> Result<NewEntry> {
    let entry: NewEntry =
        ron::from_str(text).context("parsing edited RON (did you break the syntax?)")?;

    Ok(entry)
}

// TODO: Test this
