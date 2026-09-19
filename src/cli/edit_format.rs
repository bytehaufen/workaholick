use ron::ser::PrettyConfig;
use strum::IntoEnumIterator;

use crate::domain::{RecordInput, WorkCommand};

#[derive(Debug, thiserror::Error)]
pub(crate) enum EditFormatError {
    #[error("could not serialize record to RON")]
    SerializeRON {
        #[source]
        source: ron::Error,
    },

    #[error("could not serialize template to RON")]
    SerializeTemplateRON {
        #[source]
        source: ron::Error,
    },

    #[error("could not parse edited RON")]
    Parse {
        #[source]
        source: ron::de::SpannedError,
    },
}

pub fn render_for_edit(input: &RecordInput) -> Result<String, EditFormatError> {
    let pretty_config = PrettyConfig::new();

    let ron_text = ron::ser::to_string_pretty(&input, pretty_config.clone())
        // .context("serializing record to RON")?;
        .map_err(|source| EditFormatError::SerializeRON { source })?;

    let header = "// Edit the record below.\n\
         // Save and close the editor to apply changes.\n\
         // Keep the format valid.\n\
         //\n\
         // Available Commands are:\n\n"
        .to_string();

    let templates: String = WorkCommand::iter()
        .map(|command| {
            ron::ser::to_string(&command)
                .map_err(|source| EditFormatError::SerializeTemplateRON { source })
        })
        .collect::<Result<Vec<_>, EditFormatError>>()?
        .into_iter()
        .map(|line| format!("// {line}"))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!("{header}{templates}\n{ron_text}"))
}

pub fn parse_edited(text: &str) -> Result<RecordInput, EditFormatError> {
    ron::from_str(text).map_err(|source| EditFormatError::Parse { source })
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
