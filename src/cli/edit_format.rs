use anyhow::{Context, Result};
use ron::ser::PrettyConfig;
use strum::IntoEnumIterator;

use crate::domain::{RecordInput, WorkCommand};

pub fn render_for_edit(input: &RecordInput) -> Result<String> {
    let pretty_config = PrettyConfig::new();

    let ron_text = ron::ser::to_string_pretty(&input, pretty_config.clone())
        .context("serializing record to RON")?;

    let header = "// Edit the record below.\n\
         // Save and close the editor to apply changes.\n\
         // Keep the format valid.\n\
         //\n\
         // Available Commands are:\n\n"
        .to_string();

    let templates: String = WorkCommand::iter()
        .map(|command| ron::ser::to_string(&command).context("serializing command template"))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .map(|line| format!("// {line}"))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!("{header}{templates}\n{ron_text}"))
}

pub fn parse_edited(text: &str) -> Result<RecordInput> {
    ron::from_str(text).context("parsing edited RON (did you break the syntax?)")
}

#[cfg(test)]
mod tests {
    use crate::domain::WorkRecordInput;

    use super::*;
    use chrono::{Local, TimeZone};

    #[test]
    fn render_output_parses() {
        let input = RecordInput::Work(WorkRecordInput::new(
            Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap(),
            WorkCommand::default(),
        ));

        let rendered = render_for_edit(&input).unwrap();
        let parsed = parse_edited(&rendered).unwrap();

        assert_eq!(parsed, input);
    }
}
