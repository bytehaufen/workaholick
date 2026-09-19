use crate::domain::Period;
use chrono::{DateTime, Datelike, Days, Local, NaiveDate, NaiveTime, TimeZone};

#[derive(Debug, thiserror::Error)]
pub(crate) enum ReportingError {
    #[error("cannot calculate the week start for {date}; it is outside the supported date range")]
    WeekStartOutOfRange { date: NaiveDate },

    #[error("cannot calculate the day after {date}; it is outside the supported date range")]
    NextDayOutOfRange { date: NaiveDate },

    #[error("local midnight for {date} is ambiguous or does not exist")]
    InvalidLocalMidnight { date: NaiveDate },
}

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

    pub(crate) fn dates_in(window: TimeWindow) -> impl Iterator<Item = NaiveDate> {
        let last_date = window.last_included_date();

        std::iter::successors((!window.is_empty()).then_some(window.start().date_naive()), |date| {
            date.succ_opt()
        })
        .take_while(move |date| last_date.is_some_and(|last| *date <= last))
    }

    /// Returns the last included date in this window.
    /// If the window ends at 00:00 then the previous calendar date is returned.
    pub(crate) fn last_included_date(self) -> Option<NaiveDate> {
        if self.is_empty() {
            return None;
        }

        if self.end.time() == NaiveTime::MIN {
            self.end.date_naive().pred_opt()
        } else {
            Some(self.end.date_naive())
        }
    }
}

pub(crate) fn start_of_week(date: NaiveDate) -> Result<NaiveDate, ReportingError> {
    date.checked_sub_days(Days::new(date.weekday().num_days_from_monday().into()))
        .ok_or(ReportingError::WeekStartOutOfRange { date })
}

pub(crate) fn start_of_day(date: NaiveDate) -> Result<DateTime<Local>, ReportingError> {
    Local
        .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
        .single()
        .ok_or(ReportingError::InvalidLocalMidnight { date })
}

pub(crate) fn next_day(date: NaiveDate) -> Result<NaiveDate, ReportingError> {
    date.succ_opt().ok_or(ReportingError::NextDayOutOfRange { date })
}

#[cfg(test)]
mod tests {
    use crate::test_support::{date, ts};

    use super::*;

    #[test]
    fn dates_in_excludes_the_date_starting_at_window_end() {
        let window = TimeWindow::new(ts(9, 0, 0), ts(10, 0, 0));

        let dates = TimeWindow::dates_in(window).collect::<Vec<_>>();
        assert_eq!(dates, vec![date(9)]);
    }

    #[test]
    fn dates_in_includes_a_date_when_window_during_it() {
        let window = TimeWindow::new(ts(9, 0, 0), ts(10, 12, 0));

        let dates = TimeWindow::dates_in(window).collect::<Vec<_>>();
        assert_eq!(dates, vec![date(9), date(10)]);
    }
}
