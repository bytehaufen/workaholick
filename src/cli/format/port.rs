use chrono::NaiveDate;

use crate::DayEntryView;

pub trait Formatter {
    fn print(text: &str);

    fn format_day(date: NaiveDate, entries: &[DayEntryView]) -> String;
}
