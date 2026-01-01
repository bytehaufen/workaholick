use anyhow::{Context, Result};

use crate::domain::NewEntry;

pub fn render_for_edit(entry: &NewEntry) -> Result<String> {
    let toml_text = toml::to_string_pretty(&entry).context("serializing entry to TOML")?;

    let header = "# Edit the entry below.\n\
         # Save and close the editor to apply changes.\n\
         # Keep the TOML valid.\n\n"
        .to_string();

    Ok(header + &toml_text)
}

pub fn parse_edited(text: &str) -> Result<NewEntry> {
    let entry: NewEntry =
        toml::from_str(text).context("parsing edited TOM (did you break the syntax?)")?;

    Ok(entry)
}
