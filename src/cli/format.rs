#[cfg(feature = "markdown")]
mod markdown_formatter;
mod plain_formatter;
mod port;

use std::collections::HashSet;

use anyhow::Result;
use chrono::{DateTime, Datelike, Local, NaiveDate, Timelike};

use crate::{DayOverview, config::OutputFormatter, domain::SummaryKind};

pub use plain_formatter::PlainFormatter;
pub use port::Formatter;

#[cfg(feature = "markdown")]
#[allow(unused_imports)]
pub use markdown_formatter::MarkdownFormatter;

use crate::domain::{ReportGroupBy, SummaryReport, SummaryWindow, WorkCommand, WorkItemReport};

pub fn print_day(
    formatter: OutputFormatter,
    date: NaiveDate,
    day_overview: &DayOverview,
) -> Result<()> {
    match formatter {
        OutputFormatter::Plain => {
            let rendered = PlainFormatter::format_day(date, day_overview);
            PlainFormatter::print(&rendered);
            Ok(())
        }
        OutputFormatter::Markdown => {
            #[cfg(feature = "markdown")]
            {
                let rendered = MarkdownFormatter::format_day(date, day_overview);
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

pub fn print_summary(formatter: OutputFormatter, report: &SummaryReport) -> Result<()> {
    match formatter {
        OutputFormatter::Plain => {
            let rendered = PlainFormatter::format_summary(report);
            PlainFormatter::print(&rendered);
            Ok(())
        }
        OutputFormatter::Markdown => {
            #[cfg(feature = "markdown")]
            {
                let rendered = MarkdownFormatter::format_summary(report);
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

pub fn print_report(
    formatter: OutputFormatter,
    report: &WorkItemReport,
    group_by: Option<ReportGroupBy>,
    summary: Option<&SummaryReport>,
) -> Result<()> {
    match formatter {
        OutputFormatter::Plain => {
            let rendered = PlainFormatter::format_report(report, group_by, summary);
            PlainFormatter::print(&rendered);
            Ok(())
        }
        OutputFormatter::Markdown => {
            #[cfg(feature = "markdown")]
            {
                let rendered = MarkdownFormatter::format_report(report, group_by, summary);
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

pub(super) fn format_work_command(command: &WorkCommand) -> String {
    match command {
        WorkCommand::Start { context } => {
            format!("START {}", format_summary_kind(SummaryKind::from(*context)))
        }
        WorkCommand::End { title, description } => {
            format!(
                "END   \"{}\"{}",
                escape_inline_text(title),
                format_optional_note(description.as_deref())
            )
        }
        WorkCommand::Log { title, description } => {
            format!(
                "LOG   \"{}\"{}",
                escape_inline_text(title),
                format_optional_note(description.as_deref())
            )
        }
        WorkCommand::BreakStart => "BREAK+".to_string(),
        WorkCommand::BreakEnd { description } => {
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

pub(super) fn format_summary_kind(kind: SummaryKind) -> String {
    match kind {
        SummaryKind::Work(work_context) => format!("work ({work_context})"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SummaryDisplay {
    pub title: String,
    pub rows: Vec<SummaryDisplayRow>,
    pub total: SummaryDisplayRow,
    pub weekly_days: Vec<SummaryDayDisplay>,
    pub weekly_balances: Vec<SummaryWeekDisplay>,
    pub weekly_balances_total: Option<SummaryWeekDisplay>,
    pub target: Option<String>,
    pub balance: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReportDisplay {
    pub title: String,
    pub rows: Vec<ReportItemDisplay>,
    pub groups: Vec<ReportGroupDisplay>,
    pub show_description: bool,
    pub show_group_description: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReportItemDisplay {
    pub date: String,
    pub start: String,
    pub end: String,
    pub duration: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReportGroupDisplay {
    pub title: String,
    pub description: String,
    pub count: String,
    pub duration: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SummaryDisplayRow {
    pub activity: String,
    pub day_count: String,
    pub active: String,
    pub break_time: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SummaryDayDisplay {
    pub day: String,
    pub active: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SummaryWeekDisplay {
    pub week: String,
    pub worked: String,
    pub target: String,
    pub balance: String,
}

pub(super) fn prepare_summary(report: &SummaryReport) -> SummaryDisplay {
    let mut rows = report
        .summary()
        .iter()
        .filter(|(_, bucket)| bucket.has_time())
        .map(|(kind, bucket)| {
            let active_minutes = duration_minutes(bucket.active_time());
            let break_minutes = duration_minutes(bucket.break_time());

            PreparedSummaryRow {
                activity: format_summary_kind(kind),
                day_count: format_summary_day_count(bucket.dates()),
                active_minutes,
                break_minutes,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.activity.cmp(&right.activity));

    let total_active_minutes = rows.iter().map(|row| row.active_minutes).sum();
    let total_break_minutes = rows.iter().map(|row| row.break_minutes).sum();
    let weekly_days = report
        .weekly_days()
        .iter()
        .map(|day| SummaryDayDisplay {
            day: format_summary_day(day.date()),
            active: format_summary_duration(day.active_minutes()),
        })
        .collect();
    let weekly_balances = report
        .weekly_balances()
        .iter()
        .map(|week| SummaryWeekDisplay {
            week: format_summary_week(week.start()),
            worked: format_summary_duration(week.worked_minutes()),
            target: week
                .target_minutes()
                .map(format_summary_duration)
                .unwrap_or_else(|| "n/a".to_string()),
            balance: week
                .balance_minutes()
                .map(format_signed_summary_duration)
                .unwrap_or_else(|| "n/a".to_string()),
        })
        .collect::<Vec<_>>();
    let weekly_balances_total = if report.weekly_balances().is_empty() {
        None
    } else {
        let total_worked_minutes =
            report.weekly_balances().iter().map(|week| week.worked_minutes()).sum();
        let total_target_minutes: i64 =
            report.weekly_balances().iter().filter_map(|week| week.target_minutes()).sum();
        let total_balance_minutes: i64 =
            report.weekly_balances().iter().filter_map(|week| week.balance_minutes()).sum();

        Some(SummaryWeekDisplay {
            week: "TOTAL".to_string(),
            worked: format_summary_duration(total_worked_minutes),
            target: format_summary_duration(total_target_minutes),
            balance: format_signed_summary_duration(total_balance_minutes),
        })
    };

    SummaryDisplay {
        title: format_summary_title(report.window()),
        rows: rows.into_iter().map(SummaryDisplayRow::from).collect(),
        total: SummaryDisplayRow::from(PreparedSummaryRow {
            activity: "TOTAL".to_string(),
            day_count: String::new(),
            active_minutes: total_active_minutes,
            break_minutes: total_break_minutes,
        }),
        weekly_days,
        weekly_balances,
        weekly_balances_total,
        target: report.weekly_target_minutes().map(format_summary_duration),
        balance: report.balance_minutes().map(format_signed_summary_duration),
    }
}

pub(super) fn prepare_report(
    report: &WorkItemReport,
    group_by: Option<ReportGroupBy>,
) -> ReportDisplay {
    match group_by {
        Some(ReportGroupBy::Title) => ReportDisplay {
            title: format_report_title(report.window(), group_by),
            rows: Vec::new(),
            groups: report
                .group_by_title()
                .into_iter()
                .map(|row| ReportGroupDisplay {
                    title: row.title().to_string(),
                    description: String::new(),
                    count: row.count().to_string(),
                    duration: format_summary_duration(row.duration_minutes()),
                })
                .collect(),
            show_description: false,
            show_group_description: false,
        },
        Some(ReportGroupBy::TitleDescription) => ReportDisplay {
            title: format_report_title(report.window(), group_by),
            rows: Vec::new(),
            groups: report
                .group_by_title_description()
                .into_iter()
                .map(|row| ReportGroupDisplay {
                    title: row.title().to_string(),
                    description: row.description().unwrap_or("-").to_string(),
                    count: row.count().to_string(),
                    duration: format_summary_duration(row.duration_minutes()),
                })
                .collect(),
            show_description: false,
            show_group_description: true,
        },
        None => {
            let rows = report
                .rows()
                .iter()
                .map(|row| ReportItemDisplay {
                    date: row.date().to_string(),
                    start: format_report_boundary(row.start(), row.date()),
                    end: format_report_boundary(row.end(), row.date()),
                    duration: format_summary_duration(row.duration_minutes()),
                    title: row.title().to_string(),
                    description: row.description().unwrap_or_default().to_string(),
                })
                .collect::<Vec<_>>();

            ReportDisplay {
                title: format_report_title(report.window(), group_by),
                show_description: rows.iter().any(|row| !row.description.is_empty()),
                show_group_description: false,
                rows,
                groups: Vec::new(),
            }
        }
    }
}

pub(super) fn format_summary_title(window: SummaryWindow) -> String {
    format!("Summary {}", format_summary_window(window))
}

pub(super) fn format_report_title(
    window: SummaryWindow,
    group_by: Option<ReportGroupBy>,
) -> String {
    match group_by {
        Some(ReportGroupBy::Title) => {
            format!("Report {} (grouped by title)", format_summary_window(window))
        }
        Some(ReportGroupBy::TitleDescription) => {
            format!("Report {} (grouped by title + description)", format_summary_window(window))
        }
        None => format!("Report {}", format_summary_window(window)),
    }
}

pub(super) fn format_summary_window(window: SummaryWindow) -> String {
    let start = window.start();
    let end = window.end();

    if is_midnight(start) && is_midnight(end) && end > start {
        let start_date = start.date_naive();
        let end_date = end.date_naive().pred_opt().unwrap_or_else(|| end.date_naive());

        return if start_date == end_date {
            start_date.to_string()
        } else {
            format!("{start_date} .. {end_date}")
        };
    }

    let start = format_summary_boundary(start);
    let end = format_summary_boundary(end);

    if start == end { start } else { format!("{start} .. {end}") }
}

pub(super) fn format_summary_boundary(boundary: DateTime<Local>) -> String {
    if is_midnight(boundary) {
        boundary.date_naive().to_string()
    } else {
        boundary.format("%Y-%m-%d %H:%M").to_string()
    }
}

pub(super) fn format_report_boundary(boundary: DateTime<Local>, row_date: NaiveDate) -> String {
    if boundary.date_naive() == row_date {
        boundary.format("%H:%M").to_string()
    } else {
        boundary.format("%Y-%m-%d %H:%M").to_string()
    }
}

pub(super) fn format_summary_day(date: NaiveDate) -> String {
    format!("{} {}", date.weekday(), date)
}

pub(super) fn format_summary_week(date: NaiveDate) -> String {
    let iso = date.iso_week();
    format!("{}-W{:02}", iso.year(), iso.week())
}

fn format_summary_day_count(dates: &HashSet<NaiveDate>) -> String {
    dates.len().to_string()
}

pub(super) fn format_summary_duration(total_minutes: i64) -> String {
    if total_minutes <= 0 {
        return "0m".to_string();
    }

    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;

    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }

    if parts.is_empty() { "0m".to_string() } else { parts.join(" ") }
}

pub(super) fn format_signed_summary_duration(total_minutes: i64) -> String {
    match total_minutes.cmp(&0) {
        std::cmp::Ordering::Less => format!("-{}", format_summary_duration(-total_minutes)),
        std::cmp::Ordering::Equal => "0m".to_string(),
        std::cmp::Ordering::Greater => format!("+{}", format_summary_duration(total_minutes)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedSummaryRow {
    activity: String,
    day_count: String,
    active_minutes: i64,
    break_minutes: i64,
}

impl From<PreparedSummaryRow> for SummaryDisplayRow {
    fn from(row: PreparedSummaryRow) -> Self {
        Self {
            activity: row.activity,
            day_count: row.day_count,
            active: format_summary_duration(row.active_minutes),
            break_time: format_summary_duration(row.break_minutes),
        }
    }
}

fn duration_minutes(duration: chrono::TimeDelta) -> i64 {
    duration.num_minutes().max(0)
}

fn is_midnight(boundary: DateTime<Local>) -> bool {
    boundary.hour() == 0 && boundary.minute() == 0 && boundary.second() == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::report::WorkItemRow;
    use crate::domain::{
        Record, RecordInput, SummaryDay, SummaryReport, SummaryWeek, SummaryWindow, WorkCommand,
        WorkContext, WorkItemReport, WorkRecordInput, summary,
    };
    use chrono::{Local, TimeZone};

    #[test]
    fn format_day_command_includes_optional_description() {
        let command = WorkCommand::End {
            title: "Implement day output".into(),
            description: Some("handles markdown fallback".into()),
        };

        assert_eq!(
            format_work_command(&command),
            "END   \"Implement day output\" (handles markdown fallback)"
        );
    }

    #[test]
    fn format_summary_duration_keeps_values_in_hours_and_minutes() {
        assert_eq!(format_summary_duration(0), "0m");
        assert_eq!(format_summary_duration(12), "12m");
        assert_eq!(format_summary_duration(4 * 60 + 12), "4h 12m");
        assert_eq!(format_summary_duration(2 * 24 * 60 + 4 * 60 + 12), "52h 12m");
        assert_eq!(format_summary_duration(24 * 60), "24h");
    }

    #[test]
    fn format_signed_summary_duration_includes_sign() {
        assert_eq!(format_signed_summary_duration(-(28 * 60 + 12)), "-28h 12m");
        assert_eq!(format_signed_summary_duration(0), "0m");
        assert_eq!(format_signed_summary_duration(28 * 60 + 12), "+28h 12m");
    }

    #[test]
    fn format_summary_window_uses_inclusive_dates_for_full_day_ranges() {
        let window = SummaryWindow::new(ts(10, 0, 0), ts(13, 0, 0));

        assert_eq!(format_summary_window(window), "2026-01-10 .. 2026-01-12");
    }

    #[test]
    fn format_summary_window_keeps_partial_boundaries_exact() {
        let window = SummaryWindow::new(ts(10, 0, 0), ts(12, 14, 15));

        assert_eq!(format_summary_window(window), "2026-01-10 .. 2026-01-12 14:15");
    }

    #[test]
    fn format_summary_week_uses_iso_week_label() {
        assert_eq!(
            format_summary_week(
                Local.with_ymd_and_hms(2025, 12, 29, 0, 0, 0).unwrap().date_naive()
            ),
            "2026-W01"
        );
    }

    #[test]
    fn prepare_report_renders_detailed_rows() {
        let report = WorkItemReport::new(
            SummaryWindow::new(ts(10, 0, 0), ts(11, 0, 0)),
            vec![
                WorkItemRow::new(
                    ts(10, 10, 0).date_naive(),
                    ts(10, 8, 0),
                    ts(10, 10, 0),
                    120,
                    "handoff".into(),
                    Some("sync".into()),
                ),
                WorkItemRow::new(
                    ts(10, 12, 0).date_naive(),
                    ts(10, 10, 0),
                    ts(10, 12, 0),
                    120,
                    "wrap".into(),
                    None,
                ),
            ],
        );

        let display = prepare_report(&report, None);

        assert_eq!(display.title, "Report 2026-01-10");
        assert!(display.groups.is_empty());
        assert!(display.show_description);
        assert!(!display.show_group_description);
        assert_eq!(
            display.rows,
            vec![
                ReportItemDisplay {
                    date: "2026-01-10".into(),
                    start: "08:00".into(),
                    end: "10:00".into(),
                    duration: "2h".into(),
                    title: "handoff".into(),
                    description: "sync".into(),
                },
                ReportItemDisplay {
                    date: "2026-01-10".into(),
                    start: "10:00".into(),
                    end: "12:00".into(),
                    duration: "2h".into(),
                    title: "wrap".into(),
                    description: String::new(),
                },
            ]
        );
    }

    #[test]
    fn prepare_report_groups_rows_by_title() {
        let report = WorkItemReport::new(
            SummaryWindow::new(ts(10, 0, 0), ts(11, 0, 0)),
            vec![
                WorkItemRow::new(
                    ts(10, 10, 0).date_naive(),
                    ts(10, 8, 0),
                    ts(10, 10, 0),
                    120,
                    "review".into(),
                    None,
                ),
                WorkItemRow::new(
                    ts(10, 12, 0).date_naive(),
                    ts(10, 10, 0),
                    ts(10, 12, 0),
                    60,
                    "review".into(),
                    None,
                ),
            ],
        );

        let display = prepare_report(&report, Some(ReportGroupBy::Title));

        assert_eq!(display.title, "Report 2026-01-10 (grouped by title)");
        assert!(display.rows.is_empty());
        assert!(!display.show_group_description);
        assert_eq!(
            display.groups,
            vec![ReportGroupDisplay {
                title: "review".into(),
                description: String::new(),
                count: "2".into(),
                duration: "3h".into(),
            }]
        );
    }

    #[test]
    fn prepare_report_groups_rows_by_title_description() {
        let report = WorkItemReport::new(
            SummaryWindow::new(ts(10, 0, 0), ts(11, 0, 0)),
            vec![
                WorkItemRow::new(
                    ts(10, 10, 0).date_naive(),
                    ts(10, 8, 0),
                    ts(10, 10, 0),
                    120,
                    "review".into(),
                    Some("PR #184".into()),
                ),
                WorkItemRow::new(
                    ts(10, 12, 0).date_naive(),
                    ts(10, 10, 0),
                    ts(10, 12, 0),
                    30,
                    "review".into(),
                    None,
                ),
            ],
        );

        let display = prepare_report(&report, Some(ReportGroupBy::TitleDescription));

        assert_eq!(display.title, "Report 2026-01-10 (grouped by title + description)");
        assert!(display.rows.is_empty());
        assert!(display.show_group_description);
        assert_eq!(
            display.groups,
            vec![
                ReportGroupDisplay {
                    title: "review".into(),
                    description: "PR #184".into(),
                    count: "1".into(),
                    duration: "2h".into(),
                },
                ReportGroupDisplay {
                    title: "review".into(),
                    description: "-".into(),
                    count: "1".into(),
                    duration: "30m".into(),
                },
            ]
        );
    }

    #[test]
    fn prepare_summary_sorts_rows_and_builds_total() {
        let window = SummaryWindow::new(ts(12, 0, 0), ts(15, 0, 0));
        let records = vec![
            work_record(1, ts(12, 0, 0), WorkCommand::Start { context: WorkContext::Home }),
            work_record(
                2,
                ts(12, 2, 15),
                WorkCommand::End { title: "done".into(), description: None },
            ),
            work_record(3, ts(13, 4, 0), WorkCommand::Start { context: WorkContext::Office }),
            work_record(4, ts(13, 6, 0), WorkCommand::BreakStart),
            work_record(5, ts(13, 6, 30), WorkCommand::BreakEnd { description: None }),
            work_record(
                6,
                ts(13, 8, 0),
                WorkCommand::End { title: "done".into(), description: None },
            ),
        ];
        let report = SummaryReport::with_weekly_balance(
            window,
            summary(&records, window).unwrap(),
            40 * 60,
            vec![
                SummaryDay::new(ts(12, 0, 0).date_naive(), 8 * 60),
                SummaryDay::new(ts(13, 0, 0).date_naive(), 6 * 60),
                SummaryDay::new(ts(14, 0, 0).date_naive(), 0),
            ],
        );

        let display = prepare_summary(&report);

        assert_eq!(display.title, "Summary 2026-01-12 .. 2026-01-14");
        assert_eq!(
            display.rows,
            vec![
                SummaryDisplayRow {
                    activity: "work (home)".into(),
                    day_count: "1".into(),
                    active: "2h 15m".into(),
                    break_time: "0m".into(),
                },
                SummaryDisplayRow {
                    activity: "work (office)".into(),
                    day_count: "1".into(),
                    active: "3h 30m".into(),
                    break_time: "30m".into(),
                },
            ]
        );
        assert_eq!(
            display.total,
            SummaryDisplayRow {
                activity: "TOTAL".into(),
                day_count: String::new(),
                active: "5h 45m".into(),
                break_time: "30m".into(),
            }
        );
        assert_eq!(
            display.weekly_days,
            vec![
                SummaryDayDisplay { day: "Mon 2026-01-12".into(), active: "8h".into() },
                SummaryDayDisplay { day: "Tue 2026-01-13".into(), active: "6h".into() },
                SummaryDayDisplay { day: "Wed 2026-01-14".into(), active: "0m".into() },
            ]
        );
        assert!(display.weekly_balances.is_empty());
        assert_eq!(display.weekly_balances_total, None);
        assert_eq!(display.target, Some("40h".into()));
        assert_eq!(display.balance, Some("-34h 15m".into()));
    }

    #[test]
    fn prepare_summary_includes_weekly_balance_breakdown() {
        let window = SummaryWindow::new(ts(1, 0, 0), ts(16, 0, 0));
        let report = SummaryReport::with_weekly_breakdown(
            window,
            summary(&[], window).unwrap(),
            vec![
                SummaryWeek::new(
                    Local.with_ymd_and_hms(2025, 12, 29, 0, 0, 0).unwrap().date_naive(),
                    8 * 60,
                    None,
                ),
                SummaryWeek::new(ts(5, 0, 0).date_naive(), 40 * 60, Some(40 * 60)),
            ],
        );

        let display = prepare_summary(&report);

        assert_eq!(
            display.weekly_balances,
            vec![
                SummaryWeekDisplay {
                    week: "2026-W01".into(),
                    worked: "8h".into(),
                    target: "n/a".into(),
                    balance: "n/a".into(),
                },
                SummaryWeekDisplay {
                    week: "2026-W02".into(),
                    worked: "40h".into(),
                    target: "40h".into(),
                    balance: "0m".into(),
                },
            ]
        );
        assert_eq!(
            display.weekly_balances_total,
            Some(SummaryWeekDisplay {
                week: "TOTAL".into(),
                worked: "48h".into(),
                target: "40h".into(),
                balance: "0m".into(),
            })
        );
        assert!(display.weekly_days.is_empty());
        assert_eq!(display.target, None);
        assert_eq!(display.balance, None);
    }

    fn ts(day: u32, hour: u32, minute: u32) -> chrono::DateTime<Local> {
        Local.with_ymd_and_hms(2026, 1, day, hour, minute, 0).unwrap()
    }

    fn work_record(id: u64, timestamp: chrono::DateTime<Local>, command: WorkCommand) -> Record {
        Record::from_input(id.into(), RecordInput::Work(WorkRecordInput::new(timestamp, command)))
    }
}
