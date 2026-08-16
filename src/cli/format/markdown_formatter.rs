use chrono::NaiveDate;
use termimad::Alignment;

use crate::DayOverview;
use crate::domain::{
    Timesheet,
    activity_report::{ActivityGroupBy, ActivityReport},
};

use super::Formatter;
use super::{format_work_command, prepare_activity_view, prepare_timesheet_view};

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

    fn format_timesheet(timesheet: &Timesheet) -> String {
        let timesheet_view = prepare_timesheet_view(timesheet);
        let mut output = String::new();
        let has_daily_totals = !timesheet_view.daily_totals.is_empty();
        let has_weekly_totals = !timesheet_view.weekly_totals.is_empty();
        output.push_str(&format!("# {}\n\n", timesheet_view.title));

        if timesheet_view.rows.is_empty() {
            output.push_str("_No tracked time._\n");
        } else {
            output.push_str("| Activity | Days | Active | Break |\n");
            output.push_str("| :-- | --: | --: | --: |\n");

            timesheet_view.rows.iter().chain(std::iter::once(&timesheet_view.total)).for_each(
                |row| {
                    output.push_str(&format!(
                        "| {} | {} | {} | {} |\n",
                        escape_markdown_table_cell(&row.activity),
                        row.day_count,
                        row.active,
                        row.break_time,
                    ));
                },
            );
        }
        output.push('\n');

        if has_daily_totals {
            output.push_str("| Day | Worked | Break | Absence |\n");
            output.push_str("| :-- | --: | --: | --: |\n");

            timesheet_view.daily_totals.iter().for_each(|row| {
                output.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    escape_markdown_table_cell(&row.day),
                    row.active,
                    row.break_time,
                    row.absence,
                ));
            });

            output.push('\n');
        }

        if has_weekly_totals {
            output.push_str("| Week | Worked | Break | Absence | Target | Balance |\n");
            output.push_str("| :-- | --: | --: | --: | --: | --: |\n");

            timesheet_view.weekly_totals.iter().for_each(|row| {
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

            if let Some(total) = &timesheet_view.weekly_totals_total {
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

            output.push('\n');
        }

        output.push_str("| Metric | Value |\n");
        output.push_str("| :-- | --: |\n");
        output.push_str(&format!("| Target | {} |\n", timesheet_view.target));
        output.push_str(&format!("| Balance | {} |\n", timesheet_view.balance));

        output
    }

    fn format_activity(
        activity_report: &ActivityReport,
        group_by: Option<ActivityGroupBy>,
    ) -> String {
        let activity_view = prepare_activity_view(activity_report, group_by);
        let mut output = String::new();
        output.push_str(&format!("# {}\n\n", activity_view.title));

        if !activity_view.groups.is_empty() {
            if activity_view.show_group_description {
                output.push_str("| Title | Description | Count | Duration |\n");
                output.push_str("| :-- | :-- | --: | --: |\n");

                activity_view.groups.iter().for_each(|row| {
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

                activity_view.groups.iter().for_each(|row| {
                    output.push_str(&format!(
                        "| {} | {} | {} |\n",
                        escape_markdown_table_cell(&row.title),
                        row.count,
                        row.duration,
                    ));
                });
            }
        } else if activity_view.rows.is_empty() {
            output.push_str("_No completed work items._\n");
        } else if activity_view.show_description {
            output.push_str("| Date | Start | End | Duration | Title | Description |\n");
            output.push_str("| :-- | :-- | :-- | --: | :-- | :-- |\n");

            activity_view.rows.iter().for_each(|row| {
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

            activity_view.rows.iter().for_each(|row| {
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

    use crate::domain::{AbsenceKind, AbsencePortion, WeeklySchedule};
    use crate::test_support::{absence_record, date, ts, work_record};
    use crate::{
        domain::{
            DailyTotals, Record, Timesheet, WeeklyTotals, WorkCommand, WorkContext,
            activity_report::{ActivityReport, WorkItemRow},
            calculate_time_breakdown,
            reporting::TimeWindow,
        },
        test_support::empty_schedule,
    };

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
    fn markdown_format_timesheet_renders_sorted_rows_and_totals() {
        let timesheet = timesheet(vec![
            work_record(1, 12, 0, 0, WorkCommand::Start { context: WorkContext::Home }),
            work_record(2, 12, 2, 15, WorkCommand::End { title: "done".into(), description: None }),
            work_record(3, 13, 4, 0, WorkCommand::Start { context: WorkContext::Office }),
            work_record(4, 13, 6, 0, WorkCommand::BreakStart),
            work_record(5, 13, 6, 30, WorkCommand::BreakEnd { description: None }),
            work_record(6, 13, 8, 0, WorkCommand::End { title: "done".into(), description: None }),
        ]);

        let rendered = MarkdownFormatter::format_timesheet(&timesheet);

        assert_eq!(
            &rendered,
            r#"# Timesheet 2026-01-12 .. 2026-01-14

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
| Target | 40h |
| Balance | -34h 15m |
"#
        );
    }

    #[test]
    fn markdown_format_timesheet_prints_empty_state() {
        let window = TimeWindow::new(
            Local.with_ymd_and_hms(2026, 1, 10, 0, 0, 0).unwrap(),
            Local.with_ymd_and_hms(2026, 1, 11, 0, 0, 0).unwrap(),
        );
        let timesheet = Timesheet::new(
            window,
            calculate_time_breakdown(&[], window, &empty_schedule()).unwrap(),
            0,
        );

        let rendered = MarkdownFormatter::format_timesheet(&timesheet);

        assert_eq!(
            &rendered,
            r#"# Timesheet 2026-01-10

_No tracked time._

| Metric | Value |
| :-- | --: |
| Target | 0m |
| Balance | 0m |
"#
        );
    }

    #[test]
    fn markdown_format_timesheet_renders_weekly_balance_breakdown() {
        let window = TimeWindow::new(ts(1, 0, 0), ts(12, 0, 0));
        let breakdown = calculate_time_breakdown(
            &[
                work_record(1, 1, 8, 0, WorkCommand::Start { context: WorkContext::Home }),
                work_record(
                    2,
                    1,
                    16,
                    0,
                    WorkCommand::End { title: "".to_string(), description: None },
                ),
                work_record(3, 5, 0, 0, WorkCommand::Start { context: WorkContext::Home }),
                work_record(
                    4,
                    6,
                    16,
                    0,
                    WorkCommand::End { title: "".to_string(), description: None },
                ),
            ],
            window,
            &empty_schedule(),
        )
        .unwrap();
        let timesheet = Timesheet::new(window, breakdown, 56 * 60).with_weekly_totals(vec![
            WeeklyTotals::new(
                Local.with_ymd_and_hms(2025, 12, 29, 0, 0, 0).unwrap().date_naive(),
                8 * 60,
                0,
                0,
                16 * 60,
            ),
            WeeklyTotals::new(
                Local.with_ymd_and_hms(2026, 1, 5, 0, 0, 0).unwrap().date_naive(),
                40 * 60,
                0,
                0,
                40 * 60,
            ),
        ]);

        let rendered = MarkdownFormatter::format_timesheet(&timesheet);

        assert_eq!(
            &rendered,
            r#"# Timesheet 2026-01-01 .. 2026-01-11

| Activity | Days | Active | Break |
| :-- | --: | --: | --: |
| work (home) | 3 | 48h | 0m |
| TOTAL |  | 48h | 0m |

| Week | Worked | Break | Absence | Target | Balance |
| :-- | --: | --: | --: | --: | --: |
| 2026-W01 | 8h | 0m | 0m | 16h | -8h |
| 2026-W02 | 40h | 0m | 0m | 40h | 0m |
| TOTAL | 48h | 0m | 0m | 56h | -8h |

| Metric | Value |
| :-- | --: |
| Target | 56h |
| Balance | -8h |
"#
        );
    }

    #[test]
    fn markdown_format_timesheet_renders_absence_credit() {
        let window = TimeWindow::new(ts(5, 0, 0), ts(6, 0, 0));
        let weekly_schedule = &WeeklySchedule::new([8 * 60, 0, 0, 0, 0, 0, 0]);
        let breakdown = calculate_time_breakdown(
            &[
                work_record(1, 5, 8, 0, WorkCommand::Start { context: WorkContext::Office }),
                work_record(2, 5, 10, 0, WorkCommand::BreakStart),
                work_record(3, 5, 11, 0, WorkCommand::BreakEnd { description: None }),
                work_record(4, 5, 13, 0, WorkCommand::End { title: "".into(), description: None }),
                absence_record(5, 5, AbsenceKind::Vacation, AbsencePortion::HalfDay),
            ],
            window,
            weekly_schedule,
        )
        .unwrap();

        let timesheet = Timesheet::new(window, breakdown, 8 * 60)
            .with_daily_totals(vec![DailyTotals::new(date(5), 4 * 60, 60, 4 * 60)])
            .with_weekly_totals(vec![WeeklyTotals::new(date(5), 4 * 60, 60, 4 * 60, 8 * 60)]);

        let rendered = MarkdownFormatter::format_timesheet(&timesheet);

        assert_eq!(
            &rendered,
            r#"# Timesheet 2026-01-05

| Activity | Days | Active | Break |
| :-- | --: | --: | --: |
| absence (vacation) | 1 | 4h | 0m |
| work (office) | 1 | 4h | 1h |
| TOTAL |  | 8h | 1h |

| Day | Worked | Break | Absence |
| :-- | --: | --: | --: |
| Mon 2026-01-05 | 4h | 1h | 4h |

| Week | Worked | Break | Absence | Target | Balance |
| :-- | --: | --: | --: | --: | --: |
| 2026-W02 | 4h | 1h | 4h | 8h | 0m |
| TOTAL | 4h | 1h | 4h | 8h | 0m |

| Metric | Value |
| :-- | --: |
| Target | 8h |
| Balance | 0m |
"#,
        );
    }

    #[test]
    fn markdown_format_activity_renders_detailed_rows() {
        let rendered = MarkdownFormatter::format_activity(&activity_report(), None);

        assert_eq!(
            &rendered,
            r#"# Activity 2026-01-10

| Date | Start | End | Duration | Title | Description |
| :-- | :-- | :-- | --: | :-- | :-- |
| 2026-01-10 | 08:00 | 10:00 | 2h | handoff | sync |
| 2026-01-10 | 10:00 | 12:00 | 2h | wrap |  |
"#
        );
    }

    #[test]
    fn markdown_format_activity_renders_grouped_rows() {
        let rendered =
            MarkdownFormatter::format_activity(&activity_report(), Some(ActivityGroupBy::Title));

        assert_eq!(
            &rendered,
            r#"# Activity 2026-01-10 (grouped by title)

| Title | Count | Duration |
| :-- | --: | --: |
| handoff | 1 | 2h |
| wrap | 1 | 2h |
"#
        );
    }

    #[test]
    fn markdown_format_activity_renders_grouped_rows_by_title_description() {
        let rendered = MarkdownFormatter::format_activity(
            &activity_report(),
            Some(ActivityGroupBy::TitleDescription),
        );

        assert_eq!(
            &rendered,
            r#"# Activity 2026-01-10 (grouped by title + description)

| Title | Description | Count | Duration |
| :-- | :-- | --: | --: |
| handoff | sync | 1 | 2h |
| wrap | - | 1 | 2h |
"#
        );
    }

    #[test]
    fn markdown_format_activity_prints_work_specific_empty_state() {
        let rendered = MarkdownFormatter::format_activity(
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
            r#"# Activity 2026-01-10

_No completed work items._
"#
        );
    }

    fn timesheet(records: Vec<Record>) -> Timesheet {
        let window = TimeWindow::new(
            Local.with_ymd_and_hms(2026, 1, 12, 0, 0, 0).unwrap(),
            Local.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap(),
        );
        let breakdown = calculate_time_breakdown(&records, window, &empty_schedule()).unwrap();

        Timesheet::new(window, breakdown, 40 * 60).with_daily_totals(vec![
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
        ])
    }

    fn activity_report() -> ActivityReport {
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
