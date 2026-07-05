use chrono::NaiveDate;

use crate::{
    DayOverview,
    domain::{
        Timesheet,
        activity_report::{ActivityGroupBy, ActivityReport},
    },
};

pub trait Formatter {
    fn print(text: &str);

    fn format_day(date: NaiveDate, day_overview: &DayOverview) -> String;

    fn format_summary(report: &Timesheet) -> String;

    fn format_report(report: &ActivityReport, group_by: Option<ActivityGroupBy>) -> String;
}
