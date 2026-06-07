use chrono::NaiveDate;

use super::Formatter;
use super::{format_work_command, prepare_report, prepare_summary};
use crate::DayOverview;
use crate::domain::{ReportGroupBy, SummaryReport, WorkItemReport};

pub struct PlainFormatter;

impl Formatter for PlainFormatter {
    fn print(text: &str) {
        print!("{text}");
    }

    fn format_day(date: NaiveDate, day_overview: &DayOverview) -> String {
        let mut output = String::new();
        output.push_str(&format!("Day {date}\n"));

        if day_overview.is_empty() {
            output.push_str("\nNo records.\n");
            return output;
        }

        if !day_overview.work_records.is_empty() {
            output.push('\n');
            output.push_str(&format_work_records(&day_overview.work_records));
        }

        if !day_overview.absence_records.is_empty() {
            output.push('\n');
            output.push_str(&format_absence_records(&day_overview.absence_records));
        }

        output
    }

    fn format_summary(report: &SummaryReport) -> String {
        let summary = prepare_summary(report);
        let mut output = String::new();
        output.push_str(&summary.title);
        output.push_str("\n\n");

        if summary.rows.is_empty() {
            output.push_str("No tracked time.\n");
        } else {
            let activity_width = summary
                .rows
                .iter()
                .chain(std::iter::once(&summary.total))
                .map(|row| row.activity.len())
                .max()
                .unwrap_or("Activity".len())
                .max("Activity".len());
            let day_count_width = summary
                .rows
                .iter()
                .chain(std::iter::once(&summary.total))
                .map(|row| row.day_count.len())
                .max()
                .unwrap_or("Days".len())
                .max("Days".len());
            let active_width = summary
                .rows
                .iter()
                .chain(std::iter::once(&summary.total))
                .map(|row| row.active.len())
                .max()
                .unwrap_or("Active".len())
                .max("Active".len());
            let break_width = summary
                .rows
                .iter()
                .chain(std::iter::once(&summary.total))
                .map(|row| row.break_time.len())
                .max()
                .unwrap_or("Break".len())
                .max("Break".len());
            output.push_str(&format!(
                "{:<activity_width$}  {:>day_count_width$}  {:>active_width$}  {:>break_width$}\n",
                "Activity", "Days", "Active", "Break",
            ));
            output.push_str(&format!(
                "{:-<activity_width$}  {:-<day_count_width$}  {:-<active_width$}  {:-<break_width$}\n",
                "", "", "", "",
            ));

            for row in summary.rows.iter().chain(std::iter::once(&summary.total)) {
                output.push_str(&format!(
                    "{:<activity_width$}  {:>day_count_width$}  {:>active_width$}  {:>break_width$}\n",
                    row.activity, row.day_count, row.active, row.break_time,
                ));
            }
        }

        if !summary.weekly_days.is_empty() {
            output.push('\n');

            let day_width = summary
                .weekly_days
                .iter()
                .map(|row| row.day.len())
                .max()
                .unwrap_or("Day".len())
                .max("Day".len());
            let worked_width = summary
                .weekly_days
                .iter()
                .map(|row| row.active.len())
                .max()
                .unwrap_or("Worked".len())
                .max("Worked".len());
            output.push_str(&format!("{:<day_width$}  {:>worked_width$}\n", "Day", "Worked"));
            output.push_str(&format!("{:-<day_width$}  {:-<worked_width$}\n", "", ""));

            for row in &summary.weekly_days {
                output
                    .push_str(&format!("{:<day_width$}  {:>worked_width$}\n", row.day, row.active));
            }
        }

        if !summary.weekly_balances.is_empty() {
            output.push('\n');

            let week_width = summary
                .weekly_balances
                .iter()
                .chain(summary.weekly_balances_total.iter())
                .map(|row| row.week.len())
                .max()
                .unwrap_or("Week".len())
                .max("Week".len());
            let worked_width = summary
                .weekly_balances
                .iter()
                .chain(summary.weekly_balances_total.iter())
                .map(|row| row.worked.len())
                .max()
                .unwrap_or("Worked".len())
                .max("Worked".len());
            let target_width = summary
                .weekly_balances
                .iter()
                .chain(summary.weekly_balances_total.iter())
                .map(|row| row.target.len())
                .max()
                .unwrap_or("Target".len())
                .max("Target".len());
            let balance_width = summary
                .weekly_balances
                .iter()
                .chain(summary.weekly_balances_total.iter())
                .map(|row| row.balance.len())
                .max()
                .unwrap_or("Balance".len())
                .max("Balance".len());
            output.push_str(&format!(
                "{:<week_width$}  {:>worked_width$}  {:>target_width$}  {:>balance_width$}\n",
                "Week", "Worked", "Target", "Balance",
            ));
            output.push_str(&format!(
                "{:-<week_width$}  {:-<worked_width$}  {:-<target_width$}  {:-<balance_width$}\n",
                "", "", "", "",
            ));

            for row in &summary.weekly_balances {
                output.push_str(&format!(
                    "{:<week_width$}  {:>worked_width$}  {:>target_width$}  {:>balance_width$}\n",
                    row.week, row.worked, row.target, row.balance,
                ));
            }

            if let Some(total) = &summary.weekly_balances_total {
                output.push_str(&format!(
                    "{:<week_width$}  {:>worked_width$}  {:>target_width$}  {:>balance_width$}\n",
                    total.week, total.worked, total.target, total.balance,
                ));
            }
        }

        if let (Some(target), Some(balance)) = (&summary.target, &summary.balance) {
            output.push('\n');
            output.push_str(&format!("Week Target: {}\n", target));
            output.push_str(&format!("Balance:     {}\n", balance));
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
        output.push_str(&report.title);
        output.push_str("\n\n");

        if !report.groups.is_empty() {
            let title_width = report
                .groups
                .iter()
                .map(|row| row.title.len())
                .max()
                .unwrap_or("Title".len())
                .max("Title".len());
            let description_width = report
                .groups
                .iter()
                .map(|row| row.description.len())
                .max()
                .unwrap_or("Description".len())
                .max("Description".len());
            let count_width = report
                .groups
                .iter()
                .map(|row| row.count.len())
                .max()
                .unwrap_or("Count".len())
                .max("Count".len());
            let duration_width = report
                .groups
                .iter()
                .map(|row| row.duration.len())
                .max()
                .unwrap_or("Duration".len())
                .max("Duration".len());

            if report.show_group_description {
                output.push_str(&format!(
                    "{:<title_width$}  {:<description_width$}  {:>count_width$}  {:>duration_width$}\n",
                    "Title", "Description", "Count", "Duration",
                ));
                output.push_str(&format!(
                    "{:-<title_width$}  {:-<description_width$}  {:-<count_width$}  {:-<duration_width$}\n",
                    "", "", "", "",
                ));

                for row in &report.groups {
                    let line = format!(
                        "{:<title_width$}  {:<description_width$}  {:>count_width$}  {:>duration_width$}\n",
                        row.title, row.description, row.count, row.duration,
                    );
                    output.push_str(line.trim_end());
                    output.push('\n');
                }
            } else {
                output.push_str(&format!(
                    "{:<title_width$}  {:>count_width$}  {:>duration_width$}\n",
                    "Title", "Count", "Duration",
                ));
                output.push_str(&format!(
                    "{:-<title_width$}  {:-<count_width$}  {:-<duration_width$}\n",
                    "", "", "",
                ));

                for row in &report.groups {
                    let line = format!(
                        "{:<title_width$}  {:>count_width$}  {:>duration_width$}\n",
                        row.title, row.count, row.duration,
                    );
                    output.push_str(line.trim_end());
                    output.push('\n');
                }
            }
        } else if report.rows.is_empty() {
            output.push_str("No completed work items.\n");
        } else {
            let date_width = report
                .rows
                .iter()
                .map(|row| row.date.len())
                .max()
                .unwrap_or("Date".len())
                .max("Date".len());
            let start_width = report
                .rows
                .iter()
                .map(|row| row.start.len())
                .max()
                .unwrap_or("Start".len())
                .max("Start".len());
            let end_width = report
                .rows
                .iter()
                .map(|row| row.end.len())
                .max()
                .unwrap_or("End".len())
                .max("End".len());
            let duration_width = report
                .rows
                .iter()
                .map(|row| row.duration.len())
                .max()
                .unwrap_or("Duration".len())
                .max("Duration".len());
            let title_width = report
                .rows
                .iter()
                .map(|row| row.title.len())
                .max()
                .unwrap_or("Title".len())
                .max("Title".len());
            let description_width = report
                .rows
                .iter()
                .map(|row| row.description.len())
                .max()
                .unwrap_or("Description".len())
                .max("Description".len());

            if report.show_description {
                output.push_str(&format!(
                    "{:<date_width$}  {:<start_width$}  {:<end_width$}  {:>duration_width$}  {:<title_width$}  {:<description_width$}\n",
                    "Date", "Start", "End", "Duration", "Title", "Description",
                ));
                output.push_str(&format!(
                    "{:-<date_width$}  {:-<start_width$}  {:-<end_width$}  {:-<duration_width$}  {:-<title_width$}  {:-<description_width$}\n",
                    "", "", "", "", "", "",
                ));

                for row in &report.rows {
                    let line = format!(
                        "{:<date_width$}  {:<start_width$}  {:<end_width$}  {:>duration_width$}  {:<title_width$}  {:<description_width$}\n",
                        row.date, row.start, row.end, row.duration, row.title, row.description,
                    );
                    output.push_str(line.trim_end());
                    output.push('\n');
                }
            } else {
                output.push_str(&format!(
                    "{:<date_width$}  {:<start_width$}  {:<end_width$}  {:>duration_width$}  {:<title_width$}\n",
                    "Date", "Start", "End", "Duration", "Title",
                ));
                output.push_str(&format!(
                    "{:-<date_width$}  {:-<start_width$}  {:-<end_width$}  {:-<duration_width$}  {:-<title_width$}\n",
                    "", "", "", "", "",
                ));

                for row in &report.rows {
                    let line = format!(
                        "{:<date_width$}  {:<start_width$}  {:<end_width$}  {:>duration_width$}  {:<title_width$}\n",
                        row.date, row.start, row.end, row.duration, row.title,
                    );
                    output.push_str(line.trim_end());
                    output.push('\n');
                }
            }
        }

        if let Some(summary) = summary {
            output.push('\n');
            output.push_str(&Self::format_summary(summary));
        }

        output
    }
}

fn format_work_records(records: &[crate::domain::WorkRecord]) -> String {
    let id_width = records
        .iter()
        .map(|record| record.id().value().to_string().len())
        .max()
        .unwrap_or(4)
        .max(4);

    let mut output = String::new();
    output.push_str("Work records\n");
    output.push_str(&format!("{:>5}  {:>id_width$}  Event\n", "Time", "ID", id_width = id_width));
    output.push_str(&format!("{:-<5}  {:-<id_width$}  {:-<5}\n", "", "", "", id_width = id_width));

    for record in records {
        output.push_str(&format_work_record_entry(record, id_width));
        output.push('\n');
    }

    output
}

fn format_work_record_entry(record: &crate::domain::WorkRecord, id_width: usize) -> String {
    let time = record.timestamp().format("%H:%M");
    let event = format_work_command(record.command());

    format!("{:>5}  {:>id_width$}  {event}", time, record.id().value())
}

fn format_absence_records(records: &[crate::domain::AbsenceRecord]) -> String {
    let id_width = records
        .iter()
        .map(|record| record.id().value().to_string().len())
        .max()
        .unwrap_or(2)
        .max(2);
    let kind_width =
        records.iter().map(|record| record.kind().to_string().len()).max().unwrap_or(4).max(4);
    let portion_width =
        records.iter().map(|record| record.portion().to_string().len()).max().unwrap_or(7).max(7);
    let note_width = records
        .iter()
        .map(|record| record.note().as_deref().unwrap_or("").len())
        .max()
        .unwrap_or(4)
        .max(4);

    let mut output = String::new();
    output.push_str("Absence records\n");
    output.push_str(&format!(
        "{:>id_width$}  {:<kind_width$}  {:<portion_width$}  Note\n",
        "ID", "Kind", "Portion",
    ));
    output.push_str(&format!(
        "{:-<id_width$}  {:-<kind_width$}  {:-<portion_width$}  {:-<note_width$}\n",
        "", "", "", "",
    ));

    for record in records {
        output.push_str(&format_absence_record_entry(record, id_width, kind_width, portion_width));
        output.push('\n');
    }

    output
}

fn format_absence_record_entry(
    record: &crate::domain::AbsenceRecord,
    id_width: usize,
    kind_width: usize,
    portion_width: usize,
) -> String {
    let note = record.note().as_deref().unwrap_or("");

    format!(
        "{:>id_width$}  {:<kind_width$}  {:<portion_width$}  {}",
        record.id().value(),
        record.kind(),
        record.portion(),
        note,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::report::WorkItemRow;
    use crate::domain::{
        Record, SummaryDay, SummaryReport, SummaryWeek, SummaryWindow, WorkCommand, WorkContext,
        WorkItemReport, summary,
    };
    use crate::test_support::work_record;
    use chrono::{Local, TimeZone};

    #[test]
    fn plain_format_day_includes_header_and_row() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        let entries = DayOverview {
            work_records: vec![crate::domain::WorkRecord::new(
                7.into(),
                Local.with_ymd_and_hms(2026, 1, 5, 9, 30, 0).unwrap(),
                WorkCommand::Start { context: WorkContext::Home },
            )],
            absence_records: vec![],
        };

        let rendered = PlainFormatter::format_day(date, &entries);

        assert_eq!(
            &rendered,
            r#"Day 2026-01-05

Work records
 Time    ID  Event
-----  ----  -----
09:30     7  START work (home)
"#
        );
    }

    #[test]
    fn plain_format_day_handles_large_ids() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        let entries = DayOverview {
            work_records: vec![
                crate::domain::WorkRecord::new(
                    7.into(),
                    Local.with_ymd_and_hms(2026, 1, 5, 9, 30, 0).unwrap(),
                    WorkCommand::Start { context: WorkContext::Home },
                ),
                crate::domain::WorkRecord::new(
                    u64::MAX.into(),
                    Local.with_ymd_and_hms(2026, 1, 5, 9, 30, 0).unwrap(),
                    WorkCommand::Start { context: WorkContext::Home },
                ),
            ],
            absence_records: vec![],
        };

        let rendered = PlainFormatter::format_day(date, &entries);

        assert_eq!(
            &rendered,
            r#"Day 2026-01-05

Work records
 Time                    ID  Event
-----  --------------------  -----
09:30                     7  START work (home)
09:30  18446744073709551615  START work (home)
"#
        );
    }

    #[test]
    fn plain_format_day_renders_absence_section() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        let entries = DayOverview {
            work_records: vec![crate::domain::WorkRecord::new(
                7.into(),
                Local.with_ymd_and_hms(2026, 1, 5, 9, 30, 0).unwrap(),
                WorkCommand::Start { context: WorkContext::Home },
            )],
            absence_records: vec![crate::domain::AbsenceRecord::new(
                42.into(),
                date,
                crate::domain::AbsenceKind::Vacation,
                crate::domain::AbsencePortion::HalfDay,
                Some("doctor".into()),
            )],
        };

        let rendered = PlainFormatter::format_day(date, &entries);

        assert_eq!(
            &rendered,
            r#"Day 2026-01-05

Work records
 Time    ID  Event
-----  ----  -----
09:30     7  START work (home)

Absence records
ID  Kind      Portion   Note
--  --------  --------  ------
42  vacation  half-day  doctor
"#
        );
    }

    #[test]
    fn plain_format_summary_renders_sorted_rows_and_totals() {
        let report = summary_report(vec![
            work_record(1, 12, 0, 0, WorkCommand::Start { context: WorkContext::Home }),
            work_record(2, 12, 2, 15, WorkCommand::End { title: "done".into(), description: None }),
            work_record(3, 13, 4, 0, WorkCommand::Start { context: WorkContext::Office }),
            work_record(4, 13, 6, 0, WorkCommand::BreakStart),
            work_record(5, 13, 6, 30, WorkCommand::BreakEnd { description: None }),
            work_record(6, 13, 8, 0, WorkCommand::End { title: "done".into(), description: None }),
        ]);

        let rendered = PlainFormatter::format_summary(&report);

        assert!(rendered.starts_with("Summary 2026-01-12 .. 2026-01-14\n\n"));
        assert!(rendered.contains("work (home)"));
        assert!(rendered.contains("2h 15m"));
        assert!(rendered.contains("work (office)"));
        assert!(rendered.contains("3h 30m"));
        assert!(rendered.contains("TOTAL"));
        assert!(rendered.contains("5h 45m"));
        assert!(rendered.contains("Mon 2026-01-12"));
        assert!(rendered.contains("Tue 2026-01-13"));
        assert!(rendered.contains("Wed 2026-01-14"));
        assert!(rendered.contains("Week Target: 40h"));
        assert!(rendered.contains("Balance:     -34h 15m"));
    }

    #[test]
    fn plain_format_summary_prints_empty_state() {
        let window = SummaryWindow::new(
            Local.with_ymd_and_hms(2026, 1, 10, 0, 0, 0).unwrap(),
            Local.with_ymd_and_hms(2026, 1, 11, 0, 0, 0).unwrap(),
        );
        let report = SummaryReport::new(window, summary(&[], window).unwrap());

        let rendered = PlainFormatter::format_summary(&report);

        assert_eq!(
            &rendered,
            r#"Summary 2026-01-10

No tracked time.
"#
        );
    }

    #[test]
    fn plain_format_summary_renders_weekly_balance_breakdown() {
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

        let rendered = PlainFormatter::format_summary(&report);

        assert_eq!(
            &rendered,
            r#"Summary 2026-01-01 .. 2026-01-15

No tracked time.

Week      Worked  Target  Balance
--------  ------  ------  -------
2026-W01      8h     n/a      n/a
2026-W02     40h     40h       0m
TOTAL        48h     40h       0m
"#
        );
    }

    #[test]
    fn plain_format_report_renders_detailed_rows() {
        let rendered = PlainFormatter::format_report(&detailed_report(), None, None);

        assert_eq!(
            &rendered,
            r#"Report 2026-01-10

Date        Start  End    Duration  Title    Description
----------  -----  -----  --------  -------  -----------
2026-01-10  08:00  10:00        2h  handoff  sync
2026-01-10  10:00  12:00        2h  wrap
"#
        );
    }

    #[test]
    fn plain_format_report_renders_grouped_rows_and_appends_summary() {
        let summary = SummaryReport::new(
            SummaryWindow::new(
                Local.with_ymd_and_hms(2026, 1, 10, 0, 0, 0).unwrap(),
                Local.with_ymd_and_hms(2026, 1, 11, 0, 0, 0).unwrap(),
            ),
            summary(
                &[],
                SummaryWindow::new(
                    Local.with_ymd_and_hms(2026, 1, 10, 0, 0, 0).unwrap(),
                    Local.with_ymd_and_hms(2026, 1, 11, 0, 0, 0).unwrap(),
                ),
            )
            .unwrap(),
        );
        let rendered = PlainFormatter::format_report(
            &detailed_report(),
            Some(ReportGroupBy::Title),
            Some(&summary),
        );

        assert_eq!(
            &rendered,
            r#"Report 2026-01-10 (grouped by title)

Title    Count  Duration
-------  -----  --------
handoff      1        2h
wrap         1        2h

Summary 2026-01-10

No tracked time.
"#
        );
    }

    #[test]
    fn plain_format_report_renders_grouped_rows_by_title_description() {
        let rendered = PlainFormatter::format_report(
            &detailed_report(),
            Some(ReportGroupBy::TitleDescription),
            None,
        );

        assert_eq!(
            &rendered,
            r#"Report 2026-01-10 (grouped by title + description)

Title    Description  Count  Duration
-------  -----------  -----  --------
handoff  sync             1        2h
wrap     -                1        2h
"#
        );
    }

    #[test]
    fn plain_format_report_prints_work_specific_empty_state() {
        let rendered = PlainFormatter::format_report(
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
            r#"Report 2026-01-10

No completed work items.
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
