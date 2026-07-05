mod builder;

pub(crate) use builder::build_timesheet;

use chrono::NaiveDate;

use crate::domain::{TimeBreakdown, reporting::TimeWindow};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct TimeTotals {
    worked_minutes: i64,
    break_minutes: i64,
    absence_minutes: i64,
}

impl TimeTotals {
    /// Work - Absence + Absence minutes
    fn credited_minutes(&self) -> i64 {
        self.worked_minutes - self.break_minutes + self.absence_minutes
    }

    fn balance_minutes(&self, target_minutes: i64) -> i64 {
        self.credited_minutes() - target_minutes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyTotals {
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
pub struct WeeklyTotals {
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

#[derive(Debug, Clone)]
pub struct Timesheet {
    window: TimeWindow,
    breakdown: TimeBreakdown,
    weekly_target_minutes: Option<i64>,
    daily_totals: Vec<DailyTotals>,
    weekly_totals: Vec<WeeklyTotals>,
}

impl Timesheet {
    pub(crate) fn new(window: TimeWindow, breakdown: TimeBreakdown) -> Self {
        Self {
            window,
            breakdown,
            weekly_target_minutes: None,
            daily_totals: Vec::new(),
            weekly_totals: Vec::new(),
        }
    }

    pub(crate) fn with_weekly_balance(
        window: TimeWindow,
        breakdown: TimeBreakdown,
        weekly_target_minutes: i64,
        daily_totals: Vec<DailyTotals>,
    ) -> Self {
        Self {
            window,
            breakdown,
            weekly_target_minutes: Some(weekly_target_minutes),
            daily_totals,
            weekly_totals: Vec::new(),
        }
    }

    pub(crate) fn with_weekly_breakdown(
        window: TimeWindow,
        breakdown: TimeBreakdown,
        weekly_totals: Vec<WeeklyTotals>,
    ) -> Self {
        Self {
            window,
            breakdown,
            weekly_target_minutes: None,
            daily_totals: Vec::new(),
            weekly_totals,
        }
    }

    pub(crate) fn window(&self) -> TimeWindow {
        self.window
    }

    pub(crate) fn breakdown(&self) -> &TimeBreakdown {
        &self.breakdown
    }

    pub(crate) fn weekly_target_minutes(&self) -> Option<i64> {
        self.weekly_target_minutes
    }

    pub(crate) fn worked_minutes(&self) -> i64 {
        self.breakdown.total_active_minutes()
    }

    pub(crate) fn balance_minutes(&self) -> Option<i64> {
        self.weekly_target_minutes.map(|target| self.worked_minutes() - target)
    }

    pub(crate) fn daily_totals(&self) -> &[DailyTotals] {
        &self.daily_totals
    }

    pub(crate) fn weekly_totals(&self) -> &[WeeklyTotals] {
        &self.weekly_totals
    }
}
