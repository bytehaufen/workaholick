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

    pub(crate) fn dates_in(window: TimeWindow) -> impl Iterator<Item = NaiveDate> {
        std::iter::successors((!window.is_empty()).then_some(window.start().date_naive()), |date| {
            date.succ_opt()
        })
        .take_while(move |date| start_of_day(*date) < window.end())
    }
}

pub(crate) fn start_of_week(date: NaiveDate) -> anyhow::Result<NaiveDate> {
    date.checked_sub_days(Days::new(date.weekday().num_days_from_monday().into()))
        .ok_or_else(|| anyhow::anyhow!("week start is outside      the supported date range"))
}

pub(crate) fn start_of_day(date: NaiveDate) -> DateTime<Local> {
    Local.with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0).unwrap()
}

pub(crate) fn next_day(date: NaiveDate) -> anyhow::Result<NaiveDate> {
    date.succ_opt()
        .ok_or_else(|| anyhow::anyhow!("timesheet range end should have a successor day"))
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
