use chrono::NaiveDate;

use crate::DayEntryView;

use super::Formatter;
use super::format_day_command;

pub struct MarkdownFormatter;

impl Formatter for MarkdownFormatter {
    fn print(markdown: &str) {
        let mut skin = termimad::MadSkin::default();
        for h in &mut skin.headers {
            use termimad::Alignment;

            h.align = Alignment::Left;
        }

        skin.print_text(markdown);
    }

    fn format_day(date: NaiveDate, entries: &[DayEntryView]) -> String {
        let mut output = String::new();
        output.push_str(&format!("# Day {date}\n\n"));
        output.push_str("| Time | ID | Event |\n");
        output.push_str("| ---: | ---: | :-- |\n");

        let entries_str = entries
            .iter()
            .map(|entry| {
                let event = escape_markdown_table_cell(&format_day_command(&entry.command));
                let timestamp = entry.timestamp.format("%H:%M");
                let id = entry.id;

                format!("| {timestamp} | {id} | {event} |")
            })
            .collect::<Vec<_>>()
            .join("\n");

        if !entries_str.is_empty() {
            output.push_str(&entries_str);
            output.push('\n');
        }

        output
    }
}

fn escape_markdown_table_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', "<br>")
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};

    use crate::domain::{ActivityKind, Command, WorkContext};

    use super::*;

    #[test]
    fn markdown_format_day_includes_header_and_row() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 21).unwrap();
        let entries = vec![DayEntryView {
            id: 7.into(),
            timestamp: Local.with_ymd_and_hms(2026, 3, 21, 5, 15, 12).unwrap(),
            command: Command::Start { kind: ActivityKind::Work { context: WorkContext::Home } },
        }];

        let rendered = MarkdownFormatter::format_day(date, &entries);

        assert_eq!(
            &rendered,
            r#"# Day 2026-03-21

| Time | ID | Event |
| ---: | ---: | :-- |
| 05:15 | 7 | START work (home) |
"#
        );
    }

    #[test]
    fn markdown_format_day_escapes_pipes_and_new_line() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 21).unwrap();
        let entries = vec![
            DayEntryView {
                id: 7.into(),
                timestamp: Local.with_ymd_and_hms(2026, 3, 21, 5, 15, 12).unwrap(),
                command: Command::Start { kind: ActivityKind::Work { context: WorkContext::Home } },
            },
            DayEntryView {
                id: 42.into(),
                timestamp: Local.with_ymd_and_hms(2026, 3, 22, 5, 15, 12).unwrap(),
                command: Command::End {
                    title: "End Working | but with pipes".into(),
                    description: Some(
                        "That is\nso much description,\nthat it spans multiple\nlines".into(),
                    ),
                },
            },
            DayEntryView {
                id: 42.into(),
                timestamp: Local.with_ymd_and_hms(2026, 3, 22, 5, 15, 12).unwrap(),
                command: Command::Start { kind: ActivityKind::Work { context: WorkContext::Home } },
            },
        ];

        let rendered = MarkdownFormatter::format_day(date, &entries);
        println!("{}", &rendered);

        assert_eq!(
            &rendered,
            r#"# Day 2026-03-21

| Time | ID | Event |
| ---: | ---: | :-- |
| 05:15 | 7 | START work (home) |
| 05:15 | 42 | END   "End Working \| but with pipes" (That is<br>so much description,<br>that it spans multiple<br>lines) |
| 05:15 | 42 | START work (home) |
"#
        );
    }
}
