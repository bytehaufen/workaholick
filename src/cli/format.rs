#[cfg(feature = "markdown")]
mod markdown_formatter;
mod plain_formatter;
mod port;

use anyhow::Result;
use chrono::NaiveDate;

use crate::{DayEntryView, config::OutputFormatter};

pub use plain_formatter::PlainFormatter;
pub use port::Formatter;

#[cfg(feature = "markdown")]
#[allow(unused_imports)]
pub use markdown_formatter::MarkdownFormatter;

use crate::domain::{ActivityKind, Command};

pub fn print_day(
    formatter: OutputFormatter,
    date: NaiveDate,
    entries: &[DayEntryView],
) -> Result<()> {
    match formatter {
        OutputFormatter::Plain => {
            let rendered = PlainFormatter::format_day(date, entries);
            PlainFormatter::print(&rendered);
            Ok(())
        }
        OutputFormatter::Markdown => {
            #[cfg(feature = "markdown")]
            {
                let rendered = MarkdownFormatter::format_day(date, entries);
                MarkdownFormatter::print(&rendered);
                Ok(())
            }

            #[cfg(not(feature = "markdown"))]
            {
                anyhow::bail!(
                    "Config sets formatter to `markdown`, but this binary was built without the `markdown` feature"
                )
            }
        }
    }
}

pub(super) fn format_day_command(command: &Command) -> String {
    match command {
        Command::Start { kind } => format!("START {}", format_activity_kind(kind)),
        Command::End { title, description } => {
            format!(
                "END   \"{}\"{}",
                escape_inline_text(title),
                format_optional_note(description.as_deref())
            )
        }
        Command::Log { title, description } => {
            format!(
                "LOG   \"{}\"{}",
                escape_inline_text(title),
                format_optional_note(description.as_deref())
            )
        }
        Command::BreakStart => "BREAK+".to_string(),
        Command::BreakEnd { description } => {
            format!("BREAK-{}", format_optional_note(description.as_deref()))
        }
    }
}

fn format_optional_note(text: Option<&str>) -> String {
    match text {
        Some(value) if !value.trim().is_empty() => format!(" ({})", escape_inline_text(value)),
        _ => String::new(),
    }
}

fn escape_inline_text(text: &str) -> String {
    text.replace('\\', r"\\").replace('"', r#"\""#)
}

fn format_activity_kind(kind: &ActivityKind) -> String {
    match kind {
        ActivityKind::Work { context } => format!("work ({context})"),
        ActivityKind::Vacation => "vacation".to_string(),
        ActivityKind::Holiday => "holiday".to_string(),
        ActivityKind::Sick => "sick".to_string(),
        ActivityKind::SickChild => "sick-child".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_day_command_includes_optional_description() {
        let command = Command::End {
            title: "Implement day output".into(),
            description: Some("handles markdown fallback".into()),
        };

        assert_eq!(
            format_day_command(&command),
            "END   \"Implement day output\" (handles markdown fallback)"
        );
    }
}
