mod builder;
mod totals;

pub(crate) use builder::build_timesheet;
pub(crate) use totals::{DailyTotals, WeeklyTotals};

use crate::domain::{TimeBreakdown, reporting::TimeWindow};

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
        self.breakdown.work_active_minutes()
    }

    pub(crate) fn absence_minutes(&self) -> i64 {
        self.breakdown.absence_minutes()
    }

    fn credited_minutes(&self) -> i64 {
        self.worked_minutes() + self.absence_minutes()
    }

    pub(crate) fn balance_minutes(&self) -> Option<i64> {
        self.weekly_target_minutes.map(|target| self.credited_minutes() - target)
    }

    pub(crate) fn daily_totals(&self) -> &[DailyTotals] {
        &self.daily_totals
    }

    pub(crate) fn weekly_totals(&self) -> &[WeeklyTotals] {
        &self.weekly_totals
    }
}
