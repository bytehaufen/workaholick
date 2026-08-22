#[cfg(feature = "markdown")]
mod markdown_formatter;
mod port;

use anyhow::Result;
use chrono::{DateTime, Datelike, Local, NaiveDate, Timelike};

use crate::{
    DayOverview,
    config::OutputFormatter,
    domain::{DailyTotals, TimeCategory},
};

pub use port::Formatter;

#[cfg(feature = "markdown")]
pub use markdown_formatter::MarkdownFormatter;

use crate::domain::{
    Timesheet, WorkCommand,
    activity_report::{ActivityGroupBy, ActivityReport},
    reporting::TimeWindow,
};

pub fn print_day(
    formatter: OutputFormatter,
    date: NaiveDate,
    day_overview: &DayOverview,
) -> Result<()> {
    match formatter {
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

pub fn print_timesheet(formatter: OutputFormatter, timesheet: &Timesheet) -> Result<()> {
    match formatter {
        OutputFormatter::Markdown => {
            #[cfg(feature = "markdown")]
            {
                let rendered = MarkdownFormatter::format_timesheet(timesheet);
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

pub fn print_activity(
    formatter: OutputFormatter,
    activity_report: &ActivityReport,
    group_by: Option<ActivityGroupBy>,
) -> Result<()> {
    match formatter {
        OutputFormatter::Markdown => {
            #[cfg(feature = "markdown")]
            {
                let rendered = MarkdownFormatter::format_activity(activity_report, group_by);
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
            format!("START {}", format_time_category(TimeCategory::from(*context)))
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

pub(super) fn format_time_category(category: TimeCategory) -> String {
    match category {
        TimeCategory::Work(work_context) => format!("work ({work_context})"),
        TimeCategory::Absence(absence_kind) => format!("absence ({absence_kind})"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TimesheetView {
    pub title: String,
    pub rows: Vec<TimesheetViewRow>,
    pub total: TimesheetViewRow,
    pub daily_totals: Vec<DailyTotalsView>,
    pub daily_totals_total: Option<DailyTotalsView>,
    pub weekly_totals: Vec<WeeklyTotalsView>,
    pub weekly_totals_total: Option<WeeklyTotalsView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActivityView {
    pub title: String,
    pub rows: Vec<ActivityItemView>,
    pub groups: Vec<ActivityGroupView>,
    pub show_description: bool,
    pub show_group_description: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActivityItemView {
    pub date: String,
    pub start: String,
    pub end: String,
    pub duration: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActivityGroupView {
    pub title: String,
    pub description: String,
    pub count: String,
    pub duration: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TimesheetViewRow {
    pub activity: String,
    pub day_count: usize,
    pub active: String,
    pub break_time: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DailyTotalsView {
    pub day: String,
    pub active: String,
    pub break_time: String,
    pub absence: String,
    pub target: String,
    pub balance: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WeeklyTotalsView {
    pub week: String,
    pub worked: String,
    pub break_time: String,
    pub absence: String,
    pub target: String,
    pub balance: String,
}

pub(super) fn prepare_timesheet_view(timesheet: &Timesheet) -> TimesheetView {
    let mut rows = timesheet
        .breakdown()
        .iter()
        .filter(|(_, bucket)| bucket.has_time())
        .map(|(kind, bucket)| {
            let active_minutes = duration_minutes(bucket.active_time());
            let break_minutes = duration_minutes(bucket.break_time());

            PreparedTimesheetRow {
                activity: format_time_category(kind),
                day_count: bucket.dates().len(),
                active_minutes,
                break_minutes,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.activity.cmp(&right.activity));

    let total_day_count = rows.iter().map(|row| row.day_count).sum();
    let total_active_minutes = rows.iter().map(|row| row.active_minutes).sum();
    let total_break_minutes = rows.iter().map(|row| row.break_minutes).sum();
    let daily_totals = timesheet
        .daily_totals()
        .iter()
        .map(|day| DailyTotalsView {
            day: format_day_label(day.date()),
            active: format_duration(day.worked_minutes()),
            break_time: format_duration(day.break_minutes()),
            absence: format_duration(day.absence_minutes()),
            target: format_duration(day.target_minutes()),
            balance: format_signed_duration(day.balance_minutes()),
        })
        .collect();

    let daily_totals_total = if timesheet.daily_totals().is_empty() {
        None
    } else {
        let totals = timesheet.daily_totals();
        let total_worked_minutes = totals.iter().map(DailyTotals::worked_minutes).sum();
        let total_break_minutes = totals.iter().map(DailyTotals::break_minutes).sum();
        let total_absence_minutes = totals.iter().map(DailyTotals::absence_minutes).sum();
        let total_target_minutes = totals.iter().map(DailyTotals::target_minutes).sum();
        let total_balance_minutes = totals.iter().map(DailyTotals::balance_minutes).sum();

        Some(DailyTotalsView {
            day: "TOTAL".to_string(),
            active: format_duration(total_worked_minutes),
            break_time: format_duration(total_break_minutes),
            absence: format_duration(total_absence_minutes),
            target: format_duration(total_target_minutes),
            balance: format_signed_duration(total_balance_minutes),
        })
    };

    let weekly_totals = timesheet
        .weekly_totals()
        .iter()
        .map(|week| WeeklyTotalsView {
            week: format_week_label(week.start()),
            worked: format_duration(week.worked_minutes()),
            break_time: format_duration(week.break_minutes()),
            absence: format_duration(week.absence_minutes()),
            target: format_duration(week.target_minutes()),
            balance: format_signed_duration(week.balance_minutes()),
        })
        .collect::<Vec<_>>();
    let weekly_totals_total = if timesheet.weekly_totals().is_empty() {
        None
    } else {
        let total_worked_minutes =
            timesheet.weekly_totals().iter().map(|week| week.worked_minutes()).sum();
        let total_break_minutes =
            timesheet.weekly_totals().iter().map(|week| week.break_minutes()).sum();
        let total_absence_minutes =
            timesheet.weekly_totals().iter().map(|week| week.absence_minutes()).sum();
        let total_target_minutes: i64 =
            timesheet.weekly_totals().iter().map(|week| week.target_minutes()).sum();
        let total_balance_minutes: i64 =
            timesheet.weekly_totals().iter().map(|week| week.balance_minutes()).sum();

        Some(WeeklyTotalsView {
            week: "TOTAL".to_string(),
            worked: format_duration(total_worked_minutes),
            break_time: format_duration(total_break_minutes),
            absence: format_duration(total_absence_minutes),
            target: format_duration(total_target_minutes),
            balance: format_signed_duration(total_balance_minutes),
        })
    };

    TimesheetView {
        title: format_timesheet_title(timesheet.window()),
        rows: rows.into_iter().map(TimesheetViewRow::from).collect(),
        total: TimesheetViewRow::from(PreparedTimesheetRow {
            activity: "TOTAL".to_string(),
            day_count: total_day_count,
            active_minutes: total_active_minutes,
            break_minutes: total_break_minutes,
        }),
        daily_totals,
        daily_totals_total,
        weekly_totals,
        weekly_totals_total,
    }
}

pub(super) fn prepare_activity_view(
    activity_report: &ActivityReport,
    group_by: Option<ActivityGroupBy>,
) -> ActivityView {
    match group_by {
        Some(ActivityGroupBy::Title) => ActivityView {
            title: format_activity_title(activity_report.window(), group_by),
            rows: Vec::new(),
            groups: activity_report
                .group_by_title()
                .into_iter()
                .map(|row| ActivityGroupView {
                    title: row.title().to_string(),
                    description: String::new(),
                    count: row.count().to_string(),
                    duration: format_duration(row.duration_minutes()),
                })
                .collect(),
            show_description: false,
            show_group_description: false,
        },
        Some(ActivityGroupBy::TitleDescription) => ActivityView {
            title: format_activity_title(activity_report.window(), group_by),
            rows: Vec::new(),
            groups: activity_report
                .group_by_title_description()
                .into_iter()
                .map(|row| ActivityGroupView {
                    title: row.title().to_string(),
                    description: row.description().unwrap_or("-").to_string(),
                    count: row.count().to_string(),
                    duration: format_duration(row.duration_minutes()),
                })
                .collect(),
            show_description: false,
            show_group_description: true,
        },
        None => {
            let rows = activity_report
                .rows()
                .iter()
                .map(|row| ActivityItemView {
                    date: row.date().to_string(),
                    start: format_activity_boundary(row.start(), row.date()),
                    end: format_activity_boundary(row.end(), row.date()),
                    duration: format_duration(row.duration_minutes()),
                    title: row.title().to_string(),
                    description: row.description().unwrap_or_default().to_string(),
                })
                .collect::<Vec<_>>();

            ActivityView {
                title: format_activity_title(activity_report.window(), group_by),
                show_description: rows.iter().any(|row| !row.description.is_empty()),
                show_group_description: false,
                rows,
                groups: Vec::new(),
            }
        }
    }
}

pub(super) fn format_timesheet_title(window: TimeWindow) -> String {
    format!("Timesheet {}", format_time_window(window))
}

pub(super) fn format_activity_title(
    window: TimeWindow,
    group_by: Option<ActivityGroupBy>,
) -> String {
    match group_by {
        Some(ActivityGroupBy::Title) => {
            format!("Activity {} (grouped by title)", format_time_window(window))
        }
        Some(ActivityGroupBy::TitleDescription) => {
            format!("Activity {} (grouped by title + description)", format_time_window(window))
        }
        None => format!("Activity {}", format_time_window(window)),
    }
}

pub(super) fn format_time_window(window: TimeWindow) -> String {
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

    let start = format_time_boundary(start);
    let end = format_time_boundary(end);

    if start == end { start } else { format!("{start} .. {end}") }
}

pub(super) fn format_time_boundary(boundary: DateTime<Local>) -> String {
    if is_midnight(boundary) {
        boundary.date_naive().to_string()
    } else {
        boundary.format("%Y-%m-%d %H:%M").to_string()
    }
}

pub(super) fn format_activity_boundary(boundary: DateTime<Local>, row_date: NaiveDate) -> String {
    if boundary.date_naive() == row_date {
        boundary.format("%H:%M").to_string()
    } else {
        boundary.format("%Y-%m-%d %H:%M").to_string()
    }
}

pub(super) fn format_day_label(date: NaiveDate) -> String {
    format!("{} {}", date.weekday(), date)
}

pub(super) fn format_week_label(date: NaiveDate) -> String {
    let iso = date.iso_week();
    format!("{}-W{:02}", iso.year(), iso.week())
}

pub(super) fn format_duration(total_minutes: i64) -> String {
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

pub(super) fn format_signed_duration(total_minutes: i64) -> String {
    match total_minutes.cmp(&0) {
        std::cmp::Ordering::Less => format!("-{}", format_duration(-total_minutes)),
        std::cmp::Ordering::Equal => "0m".to_string(),
        std::cmp::Ordering::Greater => format!("+{}", format_duration(total_minutes)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedTimesheetRow {
    activity: String,
    day_count: usize,
    active_minutes: i64,
    break_minutes: i64,
}

impl From<PreparedTimesheetRow> for TimesheetViewRow {
    fn from(row: PreparedTimesheetRow) -> Self {
        Self {
            activity: row.activity,
            day_count: row.day_count,
            active: format_duration(row.active_minutes),
            break_time: format_duration(row.break_minutes),
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
    use crate::domain::{
        AbsenceKind, AbsencePortion, DailyTotals, Timesheet, WeeklySchedule, WeeklyTotals,
        WorkCommand, WorkContext,
        activity_report::{ActivityReport, WorkItemRow},
        calculate_time_breakdown,
    };
    use crate::test_support::*;
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
    fn format_duration_keeps_values_in_hours_and_minutes() {
        assert_eq!(format_duration(0), "0m");
        assert_eq!(format_duration(12), "12m");
        assert_eq!(format_duration(4 * 60 + 12), "4h 12m");
        assert_eq!(format_duration(2 * 24 * 60 + 4 * 60 + 12), "52h 12m");
        assert_eq!(format_duration(24 * 60), "24h");
    }

    #[test]
    fn format_signed_duration_includes_sign() {
        assert_eq!(format_signed_duration(-(28 * 60 + 12)), "-28h 12m");
        assert_eq!(format_signed_duration(0), "0m");
        assert_eq!(format_signed_duration(28 * 60 + 12), "+28h 12m");
    }

    #[test]
    fn format_time_window_uses_inclusive_dates_for_full_day_ranges() {
        let window = TimeWindow::new(ts(10, 0, 0), ts(13, 0, 0));

        assert_eq!(format_time_window(window), "2026-01-10 .. 2026-01-12");
    }

    #[test]
    fn format_time_window_keeps_partial_boundaries_exact() {
        let window = TimeWindow::new(ts(10, 0, 0), ts(12, 14, 15));

        assert_eq!(format_time_window(window), "2026-01-10 .. 2026-01-12 14:15");
    }

    #[test]
    fn format_week_label_uses_iso_week_label() {
        assert_eq!(
            format_week_label(Local.with_ymd_and_hms(2025, 12, 29, 0, 0, 0).unwrap().date_naive()),
            "2026-W01"
        );
    }

    #[test]
    fn prepare_activity_view_renders_detailed_rows() {
        let activity_report = ActivityReport::new(
            TimeWindow::new(ts(10, 0, 0), ts(11, 0, 0)),
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

        let display = prepare_activity_view(&activity_report, None);

        assert_eq!(display.title, "Activity 2026-01-10");
        assert!(display.groups.is_empty());
        assert!(display.show_description);
        assert!(!display.show_group_description);
        assert_eq!(
            display.rows,
            vec![
                ActivityItemView {
                    date: "2026-01-10".into(),
                    start: "08:00".into(),
                    end: "10:00".into(),
                    duration: "2h".into(),
                    title: "handoff".into(),
                    description: "sync".into(),
                },
                ActivityItemView {
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
    fn prepare_activity_view_groups_rows_by_title() {
        let activity_report = ActivityReport::new(
            TimeWindow::new(ts(10, 0, 0), ts(11, 0, 0)),
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

        let display = prepare_activity_view(&activity_report, Some(ActivityGroupBy::Title));

        assert_eq!(display.title, "Activity 2026-01-10 (grouped by title)");
        assert!(display.rows.is_empty());
        assert!(!display.show_group_description);
        assert_eq!(
            display.groups,
            vec![ActivityGroupView {
                title: "review".into(),
                description: String::new(),
                count: "2".into(),
                duration: "3h".into(),
            }]
        );
    }

    #[test]
    fn prepare_activity_view_groups_rows_by_title_description() {
        let activity_report = ActivityReport::new(
            TimeWindow::new(ts(10, 0, 0), ts(11, 0, 0)),
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

        let display =
            prepare_activity_view(&activity_report, Some(ActivityGroupBy::TitleDescription));

        assert_eq!(display.title, "Activity 2026-01-10 (grouped by title + description)");
        assert!(display.rows.is_empty());
        assert!(display.show_group_description);
        assert_eq!(
            display.groups,
            vec![
                ActivityGroupView {
                    title: "review".into(),
                    description: "PR #184".into(),
                    count: "1".into(),
                    duration: "2h".into(),
                },
                ActivityGroupView {
                    title: "review".into(),
                    description: "-".into(),
                    count: "1".into(),
                    duration: "30m".into(),
                },
            ]
        );
    }

    #[test]
    fn prepare_timesheet_view_sorts_rows_and_builds_total() {
        let window = TimeWindow::new(ts(12, 0, 0), ts(15, 0, 0));
        let records = vec![
            work_record(1, 12, 0, 0, WorkCommand::Start { context: WorkContext::Home }),
            work_record(2, 12, 2, 15, WorkCommand::End { title: "done".into(), description: None }),
            work_record(3, 13, 4, 0, WorkCommand::Start { context: WorkContext::Office }),
            work_record(4, 13, 6, 0, WorkCommand::BreakStart),
            work_record(5, 13, 6, 30, WorkCommand::BreakEnd { description: None }),
            work_record(6, 13, 8, 0, WorkCommand::End { title: "done".into(), description: None }),
        ];
        let timesheet = Timesheet::new(
            window,
            calculate_time_breakdown(&records, window, &empty_schedule()).unwrap(),
        )
        .with_daily_totals(vec![
            DailyTotals::new(ts(12, 0, 0).date_naive(), 8 * 60, 0, 0, 8 * 60),
            DailyTotals::new(ts(13, 0, 0).date_naive(), 6 * 60, 0, 0, 8 * 60),
            DailyTotals::new(ts(14, 0, 0).date_naive(), 0, 0, 0, 8 * 60),
        ]);

        let display = prepare_timesheet_view(&timesheet);

        assert_eq!(display.title, "Timesheet 2026-01-12 .. 2026-01-14");
        assert_eq!(
            display.rows,
            vec![
                TimesheetViewRow {
                    activity: "work (home)".into(),
                    day_count: 1,
                    active: "2h 15m".into(),
                    break_time: "0m".into(),
                },
                TimesheetViewRow {
                    activity: "work (office)".into(),
                    day_count: 1,
                    active: "3h 30m".into(),
                    break_time: "30m".into(),
                },
            ]
        );
        assert_eq!(
            display.total,
            TimesheetViewRow {
                activity: "TOTAL".into(),
                day_count: 2,
                active: "5h 45m".into(),
                break_time: "30m".into(),
            }
        );
        assert_eq!(
            display.daily_totals,
            vec![
                DailyTotalsView {
                    day: "Mon 2026-01-12".into(),
                    active: "8h".into(),
                    break_time: "0m".into(),
                    absence: "0m".into(),
                    target: "8h".into(),
                    balance: "0m".into(),
                },
                DailyTotalsView {
                    day: "Tue 2026-01-13".into(),
                    active: "6h".into(),
                    break_time: "0m".into(),
                    absence: "0m".into(),
                    target: "8h".into(),
                    balance: "-2h".into()
                },
                DailyTotalsView {
                    day: "Wed 2026-01-14".into(),
                    active: "0m".into(),
                    break_time: "0m".into(),
                    absence: "0m".into(),
                    target: "8h".into(),
                    balance: "-8h".into()
                },
            ]
        );
        assert_eq!(
            display.daily_totals_total,
            Some(DailyTotalsView {
                day: "TOTAL".into(),
                active: "14h".into(),
                break_time: "0m".into(),
                absence: "0m".into(),
                target: "24h".into(),
                balance: "-10h".into()
            })
        );

        assert!(display.weekly_totals.is_empty());
        assert_eq!(display.weekly_totals_total, None);
    }

    #[test]
    fn prepare_timesheet_view_includes_weekly_balance_breakdown() {
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
        let timesheet = Timesheet::new(window, breakdown).with_weekly_totals(vec![
            WeeklyTotals::new(
                Local.with_ymd_and_hms(2025, 12, 29, 0, 0, 0).unwrap().date_naive(),
                8 * 60,
                0,
                0,
                16 * 60,
            ),
            WeeklyTotals::new(ts(5, 0, 0).date_naive(), 40 * 60, 0, 0, 40 * 60),
        ]);

        let display = prepare_timesheet_view(&timesheet);

        assert_eq!(
            display.weekly_totals,
            vec![
                WeeklyTotalsView {
                    week: "2026-W01".into(),
                    worked: "8h".into(),
                    break_time: "0m".into(),
                    absence: "0m".into(),
                    target: "16h".into(),
                    balance: "-8h".into(),
                },
                WeeklyTotalsView {
                    week: "2026-W02".into(),
                    worked: "40h".into(),
                    break_time: "0m".into(),
                    absence: "0m".into(),
                    target: "40h".into(),
                    balance: "0m".into(),
                },
            ]
        );
        assert_eq!(
            display.weekly_totals_total,
            Some(WeeklyTotalsView {
                week: "TOTAL".into(),
                worked: "48h".into(),
                break_time: "0m".into(),
                absence: "0m".into(),
                target: "56h".into(),
                balance: "-8h".into(),
            })
        );
        assert!(display.daily_totals.is_empty());
    }

    #[test]
    fn prepare_timesheet_view_includes_absence_credit() {
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

        let timesheet = Timesheet::new(window, breakdown)
            .with_daily_totals(vec![DailyTotals::new(date(5), 4 * 60, 60, 4 * 60, 8 * 60)])
            .with_weekly_totals(vec![WeeklyTotals::new(date(5), 4 * 60, 60, 4 * 60, 8 * 60)]);

        let display = prepare_timesheet_view(&timesheet);
        assert_eq!(
            display.rows,
            vec![
                TimesheetViewRow {
                    activity: "absence (vacation)".into(),
                    day_count: 1,
                    active: "4h".into(),
                    break_time: "0m".into(),
                },
                TimesheetViewRow {
                    activity: "work (office)".into(),
                    day_count: 1,
                    active: "4h".into(),
                    break_time: "1h".into()
                }
            ]
        );
        assert_eq!(
            display.total,
            TimesheetViewRow {
                activity: "TOTAL".into(),
                day_count: 2,
                active: "8h".into(),
                break_time: "1h".into(),
            }
        );

        assert_eq!(
            display.daily_totals,
            vec![DailyTotalsView {
                day: "Mon 2026-01-05".into(),
                active: "4h".into(),
                break_time: "1h".into(),
                absence: "4h".into(),
                target: "8h".into(),
                balance: "0m".into(),
            }]
        );
        assert_eq!(
            display.weekly_totals,
            vec![WeeklyTotalsView {
                week: "2026-W02".into(),
                worked: "4h".into(),
                break_time: "1h".into(),
                absence: "4h".into(),
                target: "8h".into(),
                balance: "0m".into(),
            }]
        );
        assert_eq!(
            display.weekly_totals_total,
            Some(WeeklyTotalsView {
                week: "TOTAL".into(),
                worked: "4h".into(),
                break_time: "1h".into(),
                absence: "4h".into(),
                target: "8h".into(),
                balance: "0m".into()
            })
        );
    }
}
