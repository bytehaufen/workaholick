use crate::domain::Period;
use chrono::{DateTime, Datelike, Days, Local, NaiveDate, TimeZone};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DateScope {
    Period { period: Period, offset: i64 },
    Range { from: NaiveDate, to: NaiveDate },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TimeWindow {
    start: DateTime<Local>,
    end: DateTime<Local>,
}

impl TimeWindow {
    pub(crate) fn new(start: DateTime<Local>, end: DateTime<Local>) -> Self {
        Self { start, end }
    }

    pub(crate) fn start(self) -> DateTime<Local> {
        self.start
    }

    pub(crate) fn end(self) -> DateTime<Local> {
        self.end
    }

    pub(crate) fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

pub(crate) fn start_of_week(date: NaiveDate) -> NaiveDate {
    date.checked_sub_days(Days::new(date.weekday().num_days_from_monday().into()))
        .expect("week start should be representable")
}

pub(crate) fn start_of_day(date: NaiveDate) -> DateTime<Local> {
    Local.with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0).unwrap()
}

pub(crate) fn next_day(date: NaiveDate) -> NaiveDate {
    date.succ_opt().expect("timesheet range end should have a successor day")
}
