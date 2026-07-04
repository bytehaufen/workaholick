use crate::domain::Period;
use chrono::{DateTime, Local, NaiveDate};

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
