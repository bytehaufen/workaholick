use chrono::NaiveDate;
use termimad::Alignment;

use crate::DayOverview;
use crate::domain::{ActivityGroupBy, ActivityReport, Timesheet};

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

    fn format_summary(report: &Timesheet) -> String {
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

            if has_weekly_days || has_weekly_balances || has_metrics {
                output.push('\n');
            }
        }

        if has_weekly_days {
            output.push_str("| Day | Worked | Break | Absence |\n");
            output.push_str("| :-- | --: | --: | --: |\n");

            summary.weekly_days.iter().for_each(|row| {
                output.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    escape_markdown_table_cell(&row.day),
                    row.active,
                    row.break_time,
                    row.absence,
                ));
            });

            if has_weekly_balances || has_metrics {
                output.push('\n');
            }
        }

        if has_weekly_balances {
            output.push_str("| Week | Worked | Break | Absence | Target | Balance |\n");
            output.push_str("| :-- | --: | --: | --: | --: | --: |\n");

            summary.weekly_balances.iter().for_each(|row| {
                output.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    escape_markdown_table_cell(&row.week),
                    row.worked,
                    row.break_time,
                    row.absence,
                    row.target,
                    row.balance,
                ));
            });

            if let Some(total) = &summary.weekly_balances_total {
                output.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    escape_markdown_table_cell(&total.week),
                    total.worked,
                    total.break_time,
                    total.absence,
                    total.target,
                    total.balance,
                ));
            }

            if has_metrics {
                output.push('\n');
            }
        }

        if let (Some(target), Some(balance)) = (&summary.target, &summary.balance) {
            output.push_str("| Metric | Value |\n");
            output.push_str("| :-- | --: |\n");
            output.push_str(&format!("| Week Target | {} |\n", target));
            output.push_str(&format!("| Balance | {} |\n", balance));
        }

        output
    }

    fn format_report(report: &ActivityReport, group_by: Option<ActivityGroupBy>) -> String {
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
        ActivityReport, DailyTotals, Record, Timesheet, WeeklyTotals, WorkCommand, WorkContext,
        calculate_time_breakdown, reporting::TimeWindow,
    };
    use crate::test_support::work_record;

    use super::*;

    #[test]
    fn markdown_format_day_includes_header_and_row() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 21).unwrap();
        let records = DayOverview {
            work_records: vec![crate::domain::WorkRecord::new(
                7.into(),
                Local.with_ymd_and_hms(2026, 3, 21, 5, 15, 12).unwrap(),
                WorkCommand::Start { context: WorkContext::Home },
            )],
            absence_records: vec![],
        };

        let rendered = MarkdownFormatter::format_day(date, &records);

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
        let records = DayOverview {
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

        let rendered = MarkdownFormatter::format_day(date, &records);

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
            work_record(1, 12, 0, 0, WorkCommand::Start { context: WorkContext::Home }),
            work_record(2, 12, 2, 15, WorkCommand::End { title: "done".into(), description: None }),
            work_record(3, 13, 4, 0, WorkCommand::Start { context: WorkContext::Office }),
            work_record(4, 13, 6, 0, WorkCommand::BreakStart),
            work_record(5, 13, 6, 30, WorkCommand::BreakEnd { description: None }),
            work_record(6, 13, 8, 0, WorkCommand::End { title: "done".into(), description: None }),
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

| Day | Worked | Break | Absence |
| :-- | --: | --: | --: |
| Mon 2026-01-12 | 8h | 0m | 0m |
| Tue 2026-01-13 | 6h | 0m | 0m |
| Wed 2026-01-14 | 0m | 0m | 0m |

| Metric | Value |
| :-- | --: |
| Week Target | 40h |
| Balance | -34h 15m |
"#
        );
    }

    #[test]
    fn markdown_format_summary_prints_empty_state() {
        let window = TimeWindow::new(
            Local.with_ymd_and_hms(2026, 1, 10, 0, 0, 0).unwrap(),
            Local.with_ymd_and_hms(2026, 1, 11, 0, 0, 0).unwrap(),
        );
        let report = Timesheet::new(window, calculate_time_breakdown(&[], window).unwrap());

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
        let window = TimeWindow::new(
            Local.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            Local.with_ymd_and_hms(2026, 1, 16, 0, 0, 0).unwrap(),
        );
        let report = Timesheet::with_weekly_breakdown(
            window,
            calculate_time_breakdown(&[], window).unwrap(),
            vec![
                WeeklyTotals::new(
                    Local.with_ymd_and_hms(2025, 12, 29, 0, 0, 0).unwrap().date_naive(),
                    8 * 60,
                    0,
                    0,
                    None,
                ),
                WeeklyTotals::new(
                    Local.with_ymd_and_hms(2026, 1, 5, 0, 0, 0).unwrap().date_naive(),
                    40 * 60,
                    0,
                    0,
                    Some(40 * 60),
                ),
            ],
        );

        let rendered = MarkdownFormatter::format_summary(&report);

        assert_eq!(
            &rendered,
            r#"# Summary 2026-01-01 .. 2026-01-15

_No tracked time._

| Week | Worked | Break | Absence | Target | Balance |
| :-- | --: | --: | --: | --: | --: |
| 2026-W01 | 8h | 0m | 0m | n/a | n/a |
| 2026-W02 | 40h | 0m | 0m | 40h | 0m |
| TOTAL | 48h | 0m | 0m | 40h | 0m |
"#
        );
    }

    #[test]
    fn markdown_format_report_renders_detailed_rows() {
        let rendered = MarkdownFormatter::format_report(&detailed_report(), None);

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
    fn markdown_format_report_renders_grouped_rows() {
        let rendered =
            MarkdownFormatter::format_report(&detailed_report(), Some(ActivityGroupBy::Title));

        assert_eq!(
            &rendered,
            r#"# Report 2026-01-10 (grouped by title)

| Title | Count | Duration |
| :-- | --: | --: |
| handoff | 1 | 2h |
| wrap | 1 | 2h |
"#
        );
    }

    #[test]
    fn markdown_format_report_renders_grouped_rows_by_title_description() {
        let rendered = MarkdownFormatter::format_report(
            &detailed_report(),
            Some(ActivityGroupBy::TitleDescription),
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
            &ActivityReport::new(
                TimeWindow::new(
                    Local.with_ymd_and_hms(2026, 1, 10, 0, 0, 0).unwrap(),
                    Local.with_ymd_and_hms(2026, 1, 11, 0, 0, 0).unwrap(),
                ),
                vec![],
            ),
            None,
        );

        assert_eq!(
            &rendered,
            r#"# Report 2026-01-10

_No completed work items._
"#
        );
    }

    fn summary_report(records: Vec<Record>) -> Timesheet {
        let window = TimeWindow::new(
            Local.with_ymd_and_hms(2026, 1, 12, 0, 0, 0).unwrap(),
            Local.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap(),
        );

        Timesheet::with_weekly_balance(
            window,
            calculate_time_breakdown(&records, window).unwrap(),
            40 * 60,
            vec![
                DailyTotals::new(
                    Local.with_ymd_and_hms(2026, 1, 12, 0, 0, 0).unwrap().date_naive(),
                    8 * 60,
                    0,
                    0,
                ),
                DailyTotals::new(
                    Local.with_ymd_and_hms(2026, 1, 13, 0, 0, 0).unwrap().date_naive(),
                    6 * 60,
                    0,
                    0,
                ),
                DailyTotals::new(
                    Local.with_ymd_and_hms(2026, 1, 14, 0, 0, 0).unwrap().date_naive(),
                    0,
                    0,
                    0,
                ),
            ],
        )
    }

    fn detailed_report() -> ActivityReport {
        ActivityReport::new(
            TimeWindow::new(
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
