use chrono::NaiveDate;

use crate::{
    DayOverview,
    domain::{ReportGroupBy, SummaryReport, WorkItemReport},
};

pub trait Formatter {
    fn print(text: &str);

    fn format_day(date: NaiveDate, day_overview: &DayOverview) -> String;

    fn format_summary(report: &SummaryReport) -> String;

    fn format_report(report: &WorkItemReport, group_by: Option<ReportGroupBy>) -> String;
}
