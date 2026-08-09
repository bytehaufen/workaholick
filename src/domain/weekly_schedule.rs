use chrono::Weekday;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WeeklySchedule {
    day_minutes: [i64; 7],
}

impl WeeklySchedule {
    pub(crate) fn new(day_minutes: [i64; 7]) -> Self {
        Self { day_minutes }
    }

    pub(crate) fn minutes_for(&self, weekday: Weekday) -> i64 {
        self.day_minutes[weekday.num_days_from_monday() as usize]
    }
}

#[cfg(test)]
mod tests {
    use chrono::Weekday;

    use super::*;

    fn weekly_schedule() -> WeeklySchedule {
        WeeklySchedule::new([1, 2, 3, 4, 5, 6, 7])
    }

    #[test]
    fn minutes_for_returns_minutes() {
        let schedule = weekly_schedule();

        assert_eq!(schedule.minutes_for(Weekday::Mon), 1);
        assert_eq!(schedule.minutes_for(Weekday::Tue), 2);
        assert_eq!(schedule.minutes_for(Weekday::Wed), 3);
        assert_eq!(schedule.minutes_for(Weekday::Thu), 4);
        assert_eq!(schedule.minutes_for(Weekday::Fri), 5);
        assert_eq!(schedule.minutes_for(Weekday::Sat), 6);
        assert_eq!(schedule.minutes_for(Weekday::Sun), 7);
    }
}
