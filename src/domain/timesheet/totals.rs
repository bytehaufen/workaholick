use chrono::NaiveDate;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct TimeTotals {
    worked_minutes: i64,
    break_minutes: i64,
    absence_minutes: i64,
}

impl TimeTotals {
    /// Work + Absence minutes
    fn credited_minutes(&self) -> i64 {
        self.worked_minutes + self.absence_minutes
    }

    fn balance_minutes(&self, target_minutes: i64) -> i64 {
        self.credited_minutes() - target_minutes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DailyTotals {
    date: NaiveDate,
    time_totals: TimeTotals,
}

impl DailyTotals {
    pub(crate) fn new(
        date: NaiveDate,
        worked_minutes: i64,
        break_minutes: i64,
        absence_minutes: i64,
    ) -> Self {
        Self { date, time_totals: TimeTotals { worked_minutes, break_minutes, absence_minutes } }
    }

    pub(crate) fn date(&self) -> NaiveDate {
        self.date
    }

    pub(crate) fn worked_minutes(&self) -> i64 {
        self.time_totals.worked_minutes
    }

    pub(crate) fn break_minutes(&self) -> i64 {
        self.time_totals.break_minutes
    }

    pub(crate) fn absence_minutes(&self) -> i64 {
        self.time_totals.absence_minutes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WeeklyTotals {
    start: NaiveDate,
    time_totals: TimeTotals,
    target_minutes: Option<i64>,
}

impl WeeklyTotals {
    pub(crate) fn new(
        start: NaiveDate,
        worked_minutes: i64,
        break_minutes: i64,
        absence_minutes: i64,
        target_minutes: Option<i64>,
    ) -> Self {
        Self {
            start,
            time_totals: TimeTotals { worked_minutes, break_minutes, absence_minutes },
            target_minutes,
        }
    }

    pub(crate) fn start(&self) -> NaiveDate {
        self.start
    }

    pub(crate) fn worked_minutes(&self) -> i64 {
        self.time_totals.worked_minutes
    }

    pub(crate) fn break_minutes(&self) -> i64 {
        self.time_totals.break_minutes
    }

    pub(crate) fn absence_minutes(&self) -> i64 {
        self.time_totals.absence_minutes
    }

    pub(crate) fn target_minutes(&self) -> Option<i64> {
        self.target_minutes
    }

    pub(crate) fn balance_minutes(&self) -> Option<i64> {
        self.target_minutes.map(|mins| self.time_totals.balance_minutes(mins))
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::date;

    use super::*;

    #[test]
    fn weekly_totals_calculates_balance_minutes() {
        let weekly_totals = WeeklyTotals::new(date(7), 4 * 60, 60, 4 * 60, Some(9 * 60));

        assert_eq!(weekly_totals.balance_minutes(), Some(-60));
    }
}
