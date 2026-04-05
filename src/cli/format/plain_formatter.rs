use chrono::NaiveDate;

use super::Formatter;
use super::{format_day_command, prepare_report, prepare_summary};
use crate::DayEntryView;
use crate::domain::{ReportGroupBy, SummaryReport, WorkItemReport};

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
        } else if report.rows.is_empty() {
            output.push_str("No completed items.\n");
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

fn format_day_entry(entry: &DayEntryView, id_width: usize) -> String {
    let time = entry.timestamp.format("%H:%M");
    let event = format_day_command(&entry.command);

    format!("{:>5}  {:>id_width$}  {event}", time, entry.id.value())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::report::WorkItemRow;
    use crate::domain::{
        ActivityKind, Command, SummaryDay, SummaryReport, SummaryWeek, SummaryWindow, WorkContext,
        WorkItemReport, summary,
    };
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

    #[test]
    fn plain_format_summary_renders_sorted_rows_and_totals() {
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

        let rendered = PlainFormatter::format_summary(&report);

        assert_eq!(
            &rendered,
            r#"Summary 2026-01-12 .. 2026-01-14

Activity     Days  Active  Break
-----------  ----  ------  -----
holiday         1  3h 30m    30m
work (home)     1  2h 15m     0m
TOTAL              5h 45m    30m

Day             Worked
--------------  ------
Mon 2026-01-12      8h
Tue 2026-01-13      6h
Wed 2026-01-14      0m

Week Target: 40h
Balance:     -34h 15m
"#
        );
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
