use chrono::NaiveDate;

use crate::{
    DayEntryView,
    domain::{ReportGroupBy, SummaryReport, WorkItemReport},
};

pub trait Formatter {
    fn print(text: &str);

    fn format_day(date: NaiveDate, entries: &[DayEntryView]) -> String;

    fn format_summary(report: &SummaryReport) -> String;

    fn format_report(
        report: &WorkItemReport,
        group_by: Option<ReportGroupBy>,
        summary: Option<&SummaryReport>,
    ) -> String;
}
