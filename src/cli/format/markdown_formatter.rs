use chrono::NaiveDate;

use crate::DayEntryView;
use crate::domain::SummaryReport;

use super::Formatter;
use super::{format_day_command, prepare_summary};

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

    fn format_summary(report: &SummaryReport) -> String {
        let summary = prepare_summary(report);
        let mut output = String::new();
        let has_weekly_days = !summary.weekly_days.is_empty();
        let has_metrics = summary.target.is_some() && summary.balance.is_some();
        output.push_str(&format!("# {}\n\n", summary.title));

        if summary.rows.is_empty() {
            output.push_str("_No tracked time._\n");
            if has_weekly_days || has_metrics {
                output.push('\n');
            }
        } else {
            output.push_str("| Activity | Active | Break |\n");
            output.push_str("| :-- | --: | --: |\n");

            for row in summary.rows.iter().chain(std::iter::once(&summary.total)) {
                output.push_str(&format!(
                    "| {} | {} | {} |\n",
                    escape_markdown_table_cell(&row.activity),
                    row.active,
                    row.break_time,
                ));
            }

            output.push('\n');
        }

        if has_weekly_days {
            output.push_str("| Day | Worked |\n");
            output.push_str("| :-- | --: |\n");

            for row in &summary.weekly_days {
                output.push_str(&format!(
                    "| {} | {} |\n",
                    escape_markdown_table_cell(&row.day),
                    row.active
                ));
            }

            output.push('\n');
        }

        if let (Some(target), Some(balance)) = (&summary.target, &summary.balance) {
            output.push_str("| Metric | Value |\n");
            output.push_str("| :-- | --: |\n");
            output.push_str(&format!("| Week Target | {} |\n", target));
            output.push_str(&format!("| Balance | {} |\n", balance));
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

    use crate::domain::{
        ActivityKind, Command, SummaryDay, SummaryReport, SummaryWindow, WorkContext, summary,
    };

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

    #[test]
    fn markdown_format_summary_renders_sorted_rows_and_totals() {
        let report = summary_report(vec![
            day_entry(
                1,
                Local.with_ymd_and_hms(2026, 1, 12, 0, 0, 0).unwrap(),
                Command::Start { kind: ActivityKind::Work { context: WorkContext::Home } },
            ),
            day_entry(
                2,
                Local.with_ymd_and_hms(2026, 1, 12, 2, 15, 0).unwrap(),
                Command::End { title: "done".into(), description: None },
            ),
            day_entry(
                3,
                Local.with_ymd_and_hms(2026, 1, 13, 4, 0, 0).unwrap(),
                Command::Start { kind: ActivityKind::Holiday },
            ),
            day_entry(
                4,
                Local.with_ymd_and_hms(2026, 1, 13, 6, 0, 0).unwrap(),
                Command::BreakStart,
            ),
            day_entry(
                5,
                Local.with_ymd_and_hms(2026, 1, 13, 6, 30, 0).unwrap(),
                Command::BreakEnd { description: None },
            ),
            day_entry(
                6,
                Local.with_ymd_and_hms(2026, 1, 13, 8, 0, 0).unwrap(),
                Command::End { title: "done".into(), description: None },
            ),
        ]);

        let rendered = MarkdownFormatter::format_summary(&report);

        assert_eq!(
            &rendered,
            r#"# Summary 2026-01-12 .. 2026-01-14

| Activity | Active | Break |
| :-- | --: | --: |
| holiday | 3h 30m | 30m |
| work (home) | 2h 15m | 0m |
| TOTAL | 5h 45m | 30m |

| Day | Worked |
| :-- | --: |
| Mon 2026-01-12 | 8h |
| Tue 2026-01-13 | 6h |
| Wed 2026-01-14 | 0m |

| Metric | Value |
| :-- | --: |
| Week Target | 40h |
| Balance | -34h 15m |
"#
        );
    }

    #[test]
    fn markdown_format_summary_prints_empty_state() {
        let window = SummaryWindow::new(
            Local.with_ymd_and_hms(2026, 1, 10, 0, 0, 0).unwrap(),
            Local.with_ymd_and_hms(2026, 1, 11, 0, 0, 0).unwrap(),
        );
        let report = SummaryReport::new(window, summary(&[], window).unwrap());

        let rendered = MarkdownFormatter::format_summary(&report);

        assert_eq!(
            &rendered,
            r#"# Summary 2026-01-10

_No tracked time._
"#
        );
    }

    fn summary_report(entries: Vec<DayEntryView>) -> SummaryReport {
        let window = SummaryWindow::new(
            Local.with_ymd_and_hms(2026, 1, 12, 0, 0, 0).unwrap(),
            Local.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap(),
        );

        SummaryReport::with_weekly_balance(
            window,
            summary(&entries, window).unwrap(),
            40 * 60,
            vec![
                SummaryDay::new(
                    Local.with_ymd_and_hms(2026, 1, 12, 0, 0, 0).unwrap().date_naive(),
                    8 * 60,
                ),
                SummaryDay::new(
                    Local.with_ymd_and_hms(2026, 1, 13, 0, 0, 0).unwrap().date_naive(),
                    6 * 60,
                ),
                SummaryDay::new(
                    Local.with_ymd_and_hms(2026, 1, 14, 0, 0, 0).unwrap().date_naive(),
                    0,
                ),
            ],
        )
    }

    fn day_entry(id: u64, timestamp: chrono::DateTime<Local>, command: Command) -> DayEntryView {
        DayEntryView { id: id.into(), timestamp, command }
    }
}
