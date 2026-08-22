use chrono::NaiveDate;

use crate::{
    DayOverview,
    domain::{Timesheet, activity_report::ActivityReport},
};

pub trait Formatter {
    fn print(text: &str);

    fn format_day(date: NaiveDate, day_overview: &DayOverview) -> String;

    fn format_timesheet(timesheet: &Timesheet) -> String;

    fn format_activity(activity_report: &ActivityReport, group_by_title: bool) -> String;
}
