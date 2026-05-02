use chrono::NaiveDate;
use termimad::Alignment;

use crate::DayOverview;
use crate::domain::{ReportGroupBy, SummaryReport, WorkItemReport};

use super::Formatter;
use super::{format_work_command, prepare_report, prepare_summary};

pub struct MarkdownFormatter;

impl Formatter for MarkdownFormatter {
    fn print(markdown: &str) {
        let mut skin = termimad::MadSkin::default();

        skin.headers.iter_mut().for_each(|header| header.align = Alignment::Left);

        skin.print_text(markdown);
    }

    fn format_day(date: NaiveDate, day_overview: &DayOverview) -> String {
        let mut output = String::new();
        output.push_str(&format!("# Day {date}\n\n"));

        let work_records_str = day_overview
            .work_records
            .iter()
            .map(|record| {
                let event = escape_markdown_table_cell(&format_work_command(record.command()));
                let timestamp = record.timestamp().format("%H:%M");
                let id = record.id();

                format!("| {timestamp} | {id} | {event} |")
            })
            .collect::<Vec<_>>()
            .join("\n");

        let absence_records_str = day_overview
            .absence_records
            .iter()
            .map(|record| {
                let kind = record.kind();
                let portion = record.portion();
                let note = escape_markdown_table_cell(record.note().as_deref().unwrap_or(""));
                let id = record.id();

                format!("| {id} | {kind} | {portion} | {note} |")
            })
            .collect::<Vec<_>>()
            .join("\n");

        if work_records_str.is_empty() && absence_records_str.is_empty() {
            output.push_str(&format!("_No records for {date}._"));
            return output;
        }

        if !work_records_str.is_empty() {
            output.push_str("## Work records\n\n");
            output.push_str("| Time | ID | Event |\n");
            output.push_str("| ---: | ---: | :-- |\n");
            output.push_str(&work_records_str);
            output.push_str("\n\n");
        }

        if !absence_records_str.is_empty() {
            output.push_str("## Absence records\n\n");
            output.push_str("| ID | Kind | Portion | Note |\n");
            output.push_str("| ---: | --: | --: | :-- |\n");
            output.push_str(&absence_records_str);
            output.push_str("\n\n");
        }

        output
    }

    fn format_summary(report: &SummaryReport) -> String {
        let summary = prepare_summary(report);
        let mut output = String::new();
        let has_weekly_days = !summary.weekly_days.is_empty();
        let has_weekly_balances = !summary.weekly_balances.is_empty();
        let has_metrics = summary.target.is_some() && summary.balance.is_some();
        output.push_str(&format!("# {}\n\n", summary.title));

        if summary.rows.is_empty() {
            output.push_str("_No tracked time._\n");
            if has_weekly_days || has_weekly_balances || has_metrics {
                output.push('\n');
            }
        } else {
            output.push_str("| Activity | Days | Active | Break |\n");
            output.push_str("| :-- | --: | --: | --: |\n");

            summary.rows.iter().chain(std::iter::once(&summary.total)).for_each(|row| {
                output.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    escape_markdown_table_cell(&row.activity),
                    row.day_count,
                    row.active,
                    row.break_time,
                ));
            });

            output.push('\n');
        }

        if has_weekly_days {
            output.push_str("| Day | Worked |\n");
            output.push_str("| :-- | --: |\n");

            summary.weekly_days.iter().for_each(|row| {
                output.push_str(&format!(
                    "| {} | {} |\n",
                    escape_markdown_table_cell(&row.day),
                    row.active
                ));
            });

            output.push('\n');
        }

        if has_weekly_balances {
            output.push_str("| Week | Worked | Target | Balance |\n");
            output.push_str("| :-- | --: | --: | --: |\n");

            summary.weekly_balances.iter().for_each(|row| {
                output.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    escape_markdown_table_cell(&row.week),
                    row.worked,
                    row.target,
                    row.balance,
                ));
            });

            if let Some(total) = &summary.weekly_balances_total {
                output.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    escape_markdown_table_cell(&total.week),
                    total.worked,
                    total.target,
                    total.balance,
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

    fn format_report(
        report: &WorkItemReport,
        group_by: Option<ReportGroupBy>,
        summary: Option<&SummaryReport>,
    ) -> String {
        let report = prepare_report(report, group_by);
        let mut output = String::new();
        output.push_str(&format!("# {}\n\n", report.title));

        if !report.groups.is_empty() {
            if report.show_group_description {
                output.push_str("| Title | Description | Count | Duration |\n");
                output.push_str("| :-- | :-- | --: | --: |\n");

                report.groups.iter().for_each(|row| {
                    output.push_str(&format!(
                        "| {} | {} | {} | {} |\n",
                        escape_markdown_table_cell(&row.title),
                        escape_markdown_table_cell(&row.description),
                        row.count,
                        row.duration,
                    ));
                });
            } else {
                output.push_str("| Title | Count | Duration |\n");
                output.push_str("| :-- | --: | --: |\n");

                report.groups.iter().for_each(|row| {
                    output.push_str(&format!(
                        "| {} | {} | {} |\n",
                        escape_markdown_table_cell(&row.title),
                        row.count,
                        row.duration,
                    ));
                });
            }
        } else if report.rows.is_empty() {
            output.push_str("_No completed work items._\n");
        } else if report.show_description {
            output.push_str("| Date | Start | End | Duration | Title | Description |\n");
            output.push_str("| :-- | :-- | :-- | --: | :-- | :-- |\n");

            report.rows.iter().for_each(|row| {
                output.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    row.date,
                    row.start,
                    row.end,
                    row.duration,
                    escape_markdown_table_cell(&row.title),
                    escape_markdown_table_cell(&row.description),
                ));
            });
        } else {
            output.push_str("| Date | Start | End | Duration | Title |\n");
            output.push_str("| :-- | :-- | :-- | --: | :-- |\n");

            report.rows.iter().for_each(|row| {
                output.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    row.date,
                    row.start,
                    row.end,
                    row.duration,
                    escape_markdown_table_cell(&row.title),
                ));
            });
        }

        if let Some(summary) = summary {
            output.push('\n');
            output.push_str(&Self::format_summary(summary));
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

    use crate::domain::report::WorkItemRow;
    use crate::domain::{
        Record, RecordInput, SummaryDay, SummaryReport, SummaryWeek, SummaryWindow, WorkCommand,
        WorkContext, WorkItemReport, WorkRecordInput, summary,
    };

    use super::*;

    #[test]
    fn markdown_format_day_includes_header_and_row() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 21).unwrap();
        let entries = DayOverview {
            date,
            work_records: vec![crate::domain::WorkRecord::new(
                7.into(),
                Local.with_ymd_and_hms(2026, 3, 21, 5, 15, 12).unwrap(),
                WorkCommand::Start { context: WorkContext::Home },
            )],
            absence_records: vec![],
        };

        let rendered = MarkdownFormatter::format_day(date, &entries);

        assert_eq!(
            &rendered,
            r#"# Day 2026-03-21

## Work records

| Time | ID | Event |
| ---: | ---: | :-- |
| 05:15 | 7 | START work (home) |

"#
        );
    }

    #[test]
    fn markdown_format_day_escapes_pipes_and_new_line() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 21).unwrap();
        let entries = DayOverview {
            date,
            work_records: vec![
                crate::domain::WorkRecord::new(
                    7.into(),
                    Local.with_ymd_and_hms(2026, 3, 21, 5, 15, 12).unwrap(),
                    WorkCommand::Start { context: WorkContext::Home },
                ),
                crate::domain::WorkRecord::new(
                    42.into(),
                    Local.with_ymd_and_hms(2026, 3, 21, 5, 15, 12).unwrap(),
                    WorkCommand::End {
                        title: "End Working | but with pipes".into(),
                        description: Some(
                            "That is\nso much description,\nthat it spans multiple\nlines".into(),
                        ),
                    },
                ),
            ],
            absence_records: vec![crate::domain::AbsenceRecord::new(
                99.into(),
                date,
                crate::domain::AbsenceKind::SickChild,
                crate::domain::AbsencePortion::HalfDay,
                Some("doctor | note\nnext line".into()),
            )],
        };

        let rendered = MarkdownFormatter::format_day(date, &entries);

        assert_eq!(
            &rendered,
            r#"# Day 2026-03-21

## Work records

| Time | ID | Event |
| ---: | ---: | :-- |
| 05:15 | 7 | START work (home) |
| 05:15 | 42 | END   "End Working \| but with pipes" (That is<br>so much description,<br>that it spans multiple<br>lines) |

## Absence records

| ID | Kind | Portion | Note |
| ---: | --: | --: | :-- |
| 99 | sick-child | half-day | doctor \| note<br>next line |

"#
        );
    }

    #[test]
    fn markdown_format_summary_renders_sorted_rows_and_totals() {
        let report = summary_report(vec![
            work_record(
                1,
                Local.with_ymd_and_hms(2026, 1, 12, 0, 0, 0).unwrap(),
                WorkCommand::Start { context: WorkContext::Home },
            ),
            work_record(
                2,
                Local.with_ymd_and_hms(2026, 1, 12, 2, 15, 0).unwrap(),
                WorkCommand::End { title: "done".into(), description: None },
            ),
            work_record(
                3,
                Local.with_ymd_and_hms(2026, 1, 13, 4, 0, 0).unwrap(),
                WorkCommand::Start { context: WorkContext::Office },
            ),
            work_record(
                4,
                Local.with_ymd_and_hms(2026, 1, 13, 6, 0, 0).unwrap(),
                WorkCommand::BreakStart,
            ),
            work_record(
                5,
                Local.with_ymd_and_hms(2026, 1, 13, 6, 30, 0).unwrap(),
                WorkCommand::BreakEnd { description: None },
            ),
            work_record(
                6,
                Local.with_ymd_and_hms(2026, 1, 13, 8, 0, 0).unwrap(),
                WorkCommand::End { title: "done".into(), description: None },
            ),
        ]);

        let rendered = MarkdownFormatter::format_summary(&report);

        assert_eq!(
            &rendered,
            r#"# Summary 2026-01-12 .. 2026-01-14

| Activity | Days | Active | Break |
| :-- | --: | --: | --: |
| work (home) | 1 | 2h 15m | 0m |
| work (office) | 1 | 3h 30m | 30m |
| TOTAL |  | 5h 45m | 30m |

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

    #[test]
    fn markdown_format_summary_renders_weekly_balance_breakdown() {
        let window = SummaryWindow::new(
            Local.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            Local.with_ymd_and_hms(2026, 1, 16, 0, 0, 0).unwrap(),
        );
        let report = SummaryReport::with_weekly_breakdown(
            window,
            summary(&[], window).unwrap(),
            vec![
                SummaryWeek::new(
                    Local.with_ymd_and_hms(2025, 12, 29, 0, 0, 0).unwrap().date_naive(),
                    8 * 60,
                    None,
                ),
                SummaryWeek::new(
                    Local.with_ymd_and_hms(2026, 1, 5, 0, 0, 0).unwrap().date_naive(),
                    40 * 60,
                    Some(40 * 60),
                ),
            ],
        );

        let rendered = MarkdownFormatter::format_summary(&report);

        assert_eq!(
            &rendered,
            r#"# Summary 2026-01-01 .. 2026-01-15

_No tracked time._

| Week | Worked | Target | Balance |
| :-- | --: | --: | --: |
| 2026-W01 | 8h | n/a | n/a |
| 2026-W02 | 40h | 40h | 0m |
| TOTAL | 48h | 40h | 0m |

"#
        );
    }

    #[test]
    fn markdown_format_report_renders_detailed_rows() {
        let rendered = MarkdownFormatter::format_report(&detailed_report(), None, None);

        assert_eq!(
            &rendered,
            r#"# Report 2026-01-10

| Date | Start | End | Duration | Title | Description |
| :-- | :-- | :-- | --: | :-- | :-- |
| 2026-01-10 | 08:00 | 10:00 | 2h | handoff | sync |
| 2026-01-10 | 10:00 | 12:00 | 2h | wrap |  |
"#
        );
    }

    #[test]
    fn markdown_format_report_renders_grouped_rows_and_summary() {
        let window = SummaryWindow::new(
            Local.with_ymd_and_hms(2026, 1, 10, 0, 0, 0).unwrap(),
            Local.with_ymd_and_hms(2026, 1, 11, 0, 0, 0).unwrap(),
        );
        let summary = SummaryReport::new(window, summary(&[], window).unwrap());
        let rendered = MarkdownFormatter::format_report(
            &detailed_report(),
            Some(ReportGroupBy::Title),
            Some(&summary),
        );

        assert_eq!(
            &rendered,
            r#"# Report 2026-01-10 (grouped by title)

| Title | Count | Duration |
| :-- | --: | --: |
| handoff | 1 | 2h |
| wrap | 1 | 2h |

# Summary 2026-01-10

_No tracked time._
"#
        );
    }

    #[test]
    fn markdown_format_report_renders_grouped_rows_by_title_description() {
        let rendered = MarkdownFormatter::format_report(
            &detailed_report(),
            Some(ReportGroupBy::TitleDescription),
            None,
        );

        assert_eq!(
            &rendered,
            r#"# Report 2026-01-10 (grouped by title + description)

| Title | Description | Count | Duration |
| :-- | :-- | --: | --: |
| handoff | sync | 1 | 2h |
| wrap | - | 1 | 2h |
"#
        );
    }

    #[test]
    fn markdown_format_report_prints_work_specific_empty_state() {
        let rendered = MarkdownFormatter::format_report(
            &WorkItemReport::new(
                SummaryWindow::new(
                    Local.with_ymd_and_hms(2026, 1, 10, 0, 0, 0).unwrap(),
                    Local.with_ymd_and_hms(2026, 1, 11, 0, 0, 0).unwrap(),
                ),
                vec![],
            ),
            None,
            None,
        );

        assert_eq!(
            &rendered,
            r#"# Report 2026-01-10

_No completed work items._
"#
        );
    }

    fn summary_report(records: Vec<Record>) -> SummaryReport {
        let window = SummaryWindow::new(
            Local.with_ymd_and_hms(2026, 1, 12, 0, 0, 0).unwrap(),
            Local.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap(),
        );

        SummaryReport::with_weekly_balance(
            window,
            summary(&records, window).unwrap(),
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

    fn work_record(id: u64, timestamp: chrono::DateTime<Local>, command: WorkCommand) -> Record {
        Record::from_input(id.into(), RecordInput::Work(WorkRecordInput::new(timestamp, command)))
    }

    fn detailed_report() -> WorkItemReport {
        WorkItemReport::new(
            SummaryWindow::new(
                Local.with_ymd_and_hms(2026, 1, 10, 0, 0, 0).unwrap(),
                Local.with_ymd_and_hms(2026, 1, 11, 0, 0, 0).unwrap(),
            ),
            vec![
                WorkItemRow::new(
                    Local.with_ymd_and_hms(2026, 1, 10, 10, 0, 0).unwrap().date_naive(),
                    Local.with_ymd_and_hms(2026, 1, 10, 8, 0, 0).unwrap(),
                    Local.with_ymd_and_hms(2026, 1, 10, 10, 0, 0).unwrap(),
                    120,
                    "handoff".into(),
                    Some("sync".into()),
                ),
                WorkItemRow::new(
                    Local.with_ymd_and_hms(2026, 1, 10, 12, 0, 0).unwrap().date_naive(),
                    Local.with_ymd_and_hms(2026, 1, 10, 10, 0, 0).unwrap(),
                    Local.with_ymd_and_hms(2026, 1, 10, 12, 0, 0).unwrap(),
                    120,
                    "wrap".into(),
                    None,
                ),
            ],
        )
    }
}
