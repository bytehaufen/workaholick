use chrono::NaiveDate;

use crate::{DayEntryView, domain::SummaryReport};

pub trait Formatter {
    fn print(text: &str);

    fn format_day(date: NaiveDate, entries: &[DayEntryView]) -> String;

    fn format_summary(report: &SummaryReport) -> String;
}
