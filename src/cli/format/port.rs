use chrono::NaiveDate;

use crate::{
    DayOverview,
    domain::{ActivityGroupBy, Timesheet, WorkItemReport},
};

pub trait Formatter {
    fn print(text: &str);

    fn format_day(date: NaiveDate, day_overview: &DayOverview) -> String;

    fn format_summary(report: &Timesheet) -> String;

    fn format_report(report: &WorkItemReport, group_by: Option<ActivityGroupBy>) -> String;
}
