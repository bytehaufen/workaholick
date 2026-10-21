use chrono::{Datelike, Days};

use crate::{
    config::RoundingConfig,
    domain::{
        DailyTotals, Period, Record, Timesheet, WeeklySchedule, WeeklyTotals,
        calculate_time_breakdown,
        reporting::{self, DateScope, TimeWindow},
        timesheet::TimesheetError,
    },
};

pub(crate) fn build_timesheet(
    records: &[Record],
    window: TimeWindow,
    scope: &DateScope,
    weekly_schedule: &WeeklySchedule,
    rounding_config: &RoundingConfig,
) -> Result<Timesheet, TimesheetError> {
    let breakdown = calculate_time_breakdown(records, window, weekly_schedule, rounding_config)?;

    let timesheet = Timesheet::new(window, breakdown);
    match scope {
        DateScope::Period { period: Period::Week, .. } => {
            let daily_totals =
                calculate_daily_totals(records, window, weekly_schedule, rounding_config)?;
            Ok(timesheet.with_daily_totals(daily_totals))
        }
        DateScope::Period { period: Period::Month | Period::Year, .. }
        | DateScope::Range { .. } => {
            let weekly_totals =
                calculate_weekly_totals(records, window, weekly_schedule, rounding_config)?;
            Ok(timesheet.with_weekly_totals(weekly_totals))
        }
    }
}

fn calculate_target_minutes(schedule: &WeeklySchedule, window: TimeWindow) -> i64 {
    TimeWindow::dates_in(window).map(|date| schedule.minutes_for(date.weekday())).sum()
}

fn calculate_weekly_totals(
    records: &[Record],
    window: TimeWindow,
    weekly_schedule: &WeeklySchedule,
    rounding_config: &RoundingConfig,
) -> Result<Vec<WeeklyTotals>, TimesheetError> {
    let first_week_start = reporting::start_of_week(window.start().date_naive())?;
    let last_date = window.last_included_date();

    std::iter::successors(Some(first_week_start), |week_start| {
        week_start.checked_add_days(Days::new(7))
    })
    .take_while(|week_start| last_date.is_some_and(|last| *week_start <= last))
    .map(|week_start| {
        let week_end = week_start
            .checked_add_days(Days::new(7))
            .ok_or(TimesheetError::WeekEndOutOfRange { week_start })?;

        let full_week_start = reporting::start_of_day(week_start)?;
        let full_week_end = reporting::start_of_day(week_end)?;

        let week_window =
            TimeWindow::new(full_week_start.max(window.start()), full_week_end.min(window.end()));

        let week_time_breakdown =
            calculate_time_breakdown(records, week_window, weekly_schedule, rounding_config)?;

        let worked_minutes = week_time_breakdown.work_active_minutes();
        let break_minutes = week_time_breakdown.break_minutes();
        let absence_minutes = week_time_breakdown.absence_minutes();

        let target_minutes = calculate_target_minutes(weekly_schedule, week_window);

        Ok(WeeklyTotals::new(
            week_start,
            worked_minutes,
            break_minutes,
            absence_minutes,
            target_minutes,
        ))
    })
    .collect()
}

fn calculate_daily_totals(
    records: &[Record],
    window: TimeWindow,
    weekly_schedule: &WeeklySchedule,
    rounding_config: &RoundingConfig,
) -> Result<Vec<DailyTotals>, TimesheetError> {
    let daily_totals = TimeWindow::dates_in(window)
        .map(|date| -> Result<DailyTotals, TimesheetError> {
            let day_start = reporting::start_of_day(date)?.max(window.start());
            let day_end = reporting::start_of_day(reporting::next_day(date)?)?.min(window.end());
            let day_window = TimeWindow::new(day_start, day_end);

            let breakdown =
                calculate_time_breakdown(records, day_window, weekly_schedule, rounding_config)?;

            Ok(DailyTotals::new(
                date,
                breakdown.work_active_minutes(),
                breakdown.break_minutes(),
                breakdown.absence_minutes(),
                weekly_schedule.minutes_for(day_start.weekday()),
            ))
        })
        .collect::<Result<Vec<_>, TimesheetError>>()?;

    Ok(daily_totals)
}
