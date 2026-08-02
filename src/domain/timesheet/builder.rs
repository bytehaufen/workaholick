use chrono::Days;

use crate::domain::{
    DailyTotals, Period, Record, Timesheet, WeeklySchedule, WeeklyTotals, calculate_time_breakdown,
    reporting::{self, DateScope, TimeWindow},
};

pub(crate) fn build_timesheet(
    records: &[Record],
    window: TimeWindow,
    scope: &DateScope,
    weekly_schedule: &WeeklySchedule,
) -> anyhow::Result<Timesheet> {
    let breakdown = calculate_time_breakdown(records, window)?;

    match scope {
        DateScope::Period { period: Period::Week, .. } => {
            let daily_totals = calculate_daily_totals(records, window)?;
            Ok(Timesheet::with_weekly_balance(
                window,
                breakdown,
                weekly_schedule.weekly_target_minutes(),
                daily_totals,
            ))
        }
        DateScope::Period { period: Period::Month | Period::Year, .. } => {
            let weekly_totals =
                calculate_weekly_totals(records, window, weekly_schedule.weekly_target_minutes())?;
            Ok(Timesheet::with_weekly_breakdown(window, breakdown, weekly_totals))
        }
        _ => Ok(Timesheet::new(window, breakdown)),
    }
}

fn calculate_weekly_totals(
    records: &[Record],
    window: TimeWindow,
    weekly_target_minutes: i64,
) -> anyhow::Result<Vec<WeeklyTotals>> {
    let first_week_start = reporting::start_of_week(window.start().date_naive());

    std::iter::successors(Some(first_week_start), |week_start| {
        week_start.checked_add_days(Days::new(7))
    })
    .take_while(|&week_start| reporting::start_of_day(week_start) < window.end())
    .map(|week_start| {
        let week_end =
            week_start.checked_add_days(Days::new(7)).expect("week end should be representable");

        let full_week_start = reporting::start_of_day(week_start);
        let full_week_end = reporting::start_of_day(week_end);

        let week_window =
            TimeWindow::new(full_week_start.max(window.start()), full_week_end.min(window.end()));

        let week_time_breakdown = calculate_time_breakdown(records, week_window)?;

        let worked_minutes = week_time_breakdown.work_active_minutes();
        let break_minutes = week_time_breakdown.break_minutes();
        let absence_minutes = week_time_breakdown.absence_minutes();

        Ok(WeeklyTotals::new(
            week_start,
            worked_minutes,
            break_minutes,
            absence_minutes,
            (week_window.start() == full_week_start && week_window.end() == full_week_end)
                .then_some(weekly_target_minutes),
        ))
    })
    .collect()
}

fn calculate_daily_totals(
    records: &[Record],
    window: TimeWindow,
) -> anyhow::Result<Vec<DailyTotals>> {
    let first_date = window.start().date_naive();
    let last_date = window.end().date_naive();

    let daily_totals =
        std::iter::successors(Some(first_date), |date| Some(reporting::next_day(*date)))
            .take_while(|date| *date <= last_date)
            .map(|date| -> anyhow::Result<Option<DailyTotals>> {
                let day_start = reporting::start_of_day(date).max(window.start());
                let day_end = reporting::start_of_day(reporting::next_day(date)).min(window.end());

                if day_end > day_start {
                    let day_window = TimeWindow::new(day_start, day_end);
                    let day_time_breakdown = calculate_time_breakdown(records, day_window)?;
                    let worked_minutes = day_time_breakdown.work_active_minutes();
                    let break_minutes = day_time_breakdown.break_minutes();
                    let absence_minutes = day_time_breakdown.absence_minutes();

                    Ok(Some(DailyTotals::new(date, worked_minutes, break_minutes, absence_minutes)))
                } else {
                    Ok(None)
                }
            })
            .collect::<anyhow::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();

    Ok(daily_totals)
}
