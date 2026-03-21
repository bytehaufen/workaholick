use chrono::NaiveDate;

use super::Formatter;
use super::format_day_command;
use crate::DayEntryView;

pub struct PlainFormatter;

impl Formatter for PlainFormatter {
    fn print(text: &str) {
        print!("{text}");
    }

    fn format_day(date: NaiveDate, entries: &[DayEntryView]) -> String {
        let id_width =
            entries.iter().map(|e| e.id.value().to_string().len()).max().unwrap_or(4).max(4);

        let mut output = String::new();
        output.push_str(&format!("Day {date}\n"));
        output.push_str(&format!(
            "{:>5}  {:>id_width$}  Event\n",
            "Time",
            "ID",
            id_width = id_width
        ));
        output.push_str(&format!(
            "{:-<5}  {:-<id_width$}  {:-<5}\n",
            "",
            "",
            "",
            id_width = id_width
        ));

        let entries_str = &entries
            .iter()
            .map(|entry| format_day_entry(entry, id_width))
            .collect::<Vec<_>>()
            .join("\n");

        if !entries_str.is_empty() {
            output.push_str(entries_str);
            output.push('\n');
        }

        output
    }
}

fn format_day_entry(entry: &DayEntryView, id_width: usize) -> String {
    let time = entry.timestamp.format("%H:%M");
    let event = format_day_command(&entry.command);

    format!("{:>5}  {:>id_width$}  {event}", time, entry.id.value())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ActivityKind, Command};
    use chrono::{Local, TimeZone};

    #[test]
    fn plain_format_day_includes_header_and_row() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        let entries = vec![DayEntryView {
            id: 7.into(),
            timestamp: Local.with_ymd_and_hms(2026, 1, 5, 9, 30, 0).unwrap(),
            command: Command::Start { kind: ActivityKind::Work { context: Default::default() } },
        }];

        let rendered = PlainFormatter::format_day(date, &entries);

        assert_eq!(
            &rendered,
            r#"Day 2026-01-05
 Time    ID  Event
-----  ----  -----
09:30     7  START work (home)
"#
        );
    }

    #[test]
    fn plain_format_day_handles_large_ids() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        let entries = vec![
            DayEntryView {
                id: 7.into(),
                timestamp: Local.with_ymd_and_hms(2026, 1, 5, 9, 30, 0).unwrap(),
                command: Command::Start {
                    kind: ActivityKind::Work { context: Default::default() },
                },
            },
            DayEntryView {
                id: u64::MAX.into(),
                timestamp: Local.with_ymd_and_hms(2026, 1, 5, 9, 30, 0).unwrap(),
                command: Command::Start {
                    kind: ActivityKind::Work { context: Default::default() },
                },
            },
        ];

        let rendered = PlainFormatter::format_day(date, &entries);
        println!("{rendered}");

        assert_eq!(
            &rendered,
            r#"Day 2026-01-05
 Time                    ID  Event
-----  --------------------  -----
09:30                     7  START work (home)
09:30  18446744073709551615  START work (home)
"#
        );
    }
}
