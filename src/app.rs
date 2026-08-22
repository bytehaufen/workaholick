use chrono::{DateTime, Datelike, Days, Local, Months, NaiveDate};

use crate::config::Config;
use crate::domain::WeeklySchedule;
use crate::domain::{
    AbsenceRecord, AbsenceRecordInput, ChangeOutcome, Period, Record, RecordChange, RecordId,
    RecordInput, Timesheet, WorkCommand, WorkContext, WorkRecord,
    activity_report::{self, ActivityReport},
    build_timesheet,
    reporting::{self, DateScope, TimeWindow},
    validate_records,
};
use crate::store::RecordRepository;

use anyhow::{Context, Result};

pub trait Clock {
    fn today(&self) -> NaiveDate;
    fn now(&self) -> DateTime<Local>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Local> {
        Local::now()
    }

    fn today(&self) -> NaiveDate {
        self.now().date_naive()
    }
}

#[derive(Debug)]
pub(crate) struct App<S: RecordRepository, C: Clock = SystemClock> {
    weekly_schedule: WeeklySchedule,
    store: S,
    clock: C,
}

impl<STORE: RecordRepository> App<STORE, SystemClock> {
    pub fn new(config: Config, store: STORE) -> Result<Self> {
        Self::new_with_clock(config, store, SystemClock)
    }
}

impl<STORE: RecordRepository, C: Clock> App<STORE, C> {
    pub fn new_with_clock(config: Config, store: STORE, clock: C) -> Result<Self> {
        validate_records(store.records())?;

        Ok(Self { weekly_schedule: config.schedule.to_weekly_schedule(), store, clock })
    }

    ///// Work

    pub fn start_work(&mut self, context: WorkContext) -> anyhow::Result<RecordId> {
        let input = RecordInput::new_work(self.clock.now(), WorkCommand::Start { context });

        self.insert_record(input)
    }

    pub fn end_work(
        &mut self,
        title: String,
        description: Option<String>,
    ) -> anyhow::Result<RecordId> {
        let input =
            RecordInput::new_work(self.clock.now(), WorkCommand::End { title, description });

        self.insert_record(input)
    }

    pub fn log_work(
        &mut self,
        title: String,
        description: Option<String>,
    ) -> anyhow::Result<RecordId> {
        let input =
            RecordInput::new_work(self.clock.now(), WorkCommand::Log { title, description });
        self.insert_record(input)
    }

    pub fn start_break(&mut self) -> anyhow::Result<RecordId> {
        let input = RecordInput::new_work(self.clock.now(), WorkCommand::BreakStart);
        self.insert_record(input)
    }

    pub fn end_break(&mut self, description: Option<String>) -> anyhow::Result<RecordId> {
        let input = RecordInput::new_work(self.clock.now(), WorkCommand::BreakEnd { description });
        self.insert_record(input)
    }

    ///// Absence
    pub fn add_absence(&mut self, input: AbsenceRecordInput) -> anyhow::Result<RecordId> {
        let input = RecordInput::Absence(input);
        self.insert_record(input)
    }

    ///// Common
    pub fn get_record_input(&self, id: RecordId) -> Option<RecordInput> {
        let record = self.store.records().get(&id);

        record.map(RecordInput::from_record)
    }

    pub fn update_record(&mut self, id: RecordId, input: RecordInput) -> anyhow::Result<Record> {
        match self.apply_validated_change(RecordChange::Update { id, input })? {
            ChangeOutcome::Updated { previous } => Ok(previous),
            _ => unreachable!("update change must return update outcome"),
        }
    }

    pub fn delete_record(&mut self, id: RecordId) -> anyhow::Result<Record> {
        match self.apply_validated_change(RecordChange::Delete { id })? {
            ChangeOutcome::Deleted { record } => Ok(record),
            _ => unreachable!("delete change must return delete outcome"),
        }
    }

    pub fn day(&self, date: NaiveDate) -> DayOverview {
        DayOverview::from_records(date, self.store.records().iter())
    }

    pub fn last_record_date(&self) -> Option<NaiveDate> {
        self.store
            .records()
            .iter()
            .map(|record| match record {
                Record::Work(work_record) => work_record.date(),
                Record::Absence(absence_record) => *absence_record.date(),
            })
            .max()
    }

    pub fn timesheet(&self, scope: DateScope) -> anyhow::Result<Timesheet> {
        let window = self.time_window_for(&scope)?;
        let records = self.store.records().iter().cloned().collect::<Vec<_>>();

        build_timesheet(&records, window, &scope, &self.weekly_schedule)
    }

    pub fn activity(&self, scope: DateScope) -> anyhow::Result<ActivityReport> {
        let window = self.time_window_for(&scope)?;
        let records = self.store.records().iter().cloned().collect::<Vec<_>>();

        Ok(activity_report::calculate_activity_report(&records, window)?)
    }

    //////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

    fn next_id(&self) -> RecordId {
        self.store.records().next_id()
    }

    fn insert_record(&mut self, input: RecordInput) -> anyhow::Result<RecordId> {
        match self.apply_validated_change(RecordChange::Insert { id: self.next_id(), input })? {
            ChangeOutcome::Inserted { id } => Ok(id),
            _ => unreachable!("insert change must return insert outcome"),
        }
    }

    fn apply_validated_change(&mut self, change: RecordChange) -> anyhow::Result<ChangeOutcome> {
        let mut candidate = self.store.records().clone();
        let _ = candidate.apply_change(&change)?;
        validate_records(&candidate)?;

        self.store.commit(change)
    }

    fn time_window_for(&self, scope: &DateScope) -> anyhow::Result<TimeWindow> {
        let now = self.clock.now();
        let today = self.clock.today();

        let window = match scope {
            DateScope::Period { period, offset } => {
                period_time_window(*period, *offset, today, now)?
            }
            DateScope::Range { from, to } => {
                anyhow::ensure!(
                    *from <= *to,
                    "Timesheet start date must be on or before end date."
                );
                anyhow::ensure!(*from <= today, "Timesheet start date is in the future.");
                let end = reporting::next_day(*to);

                date_window(*from, end, now)
            }
        };

        anyhow::ensure!(!window.is_empty(), "Timesheet range is empty.");

        Ok(window)
    }
}

pub struct DayOverview {
    pub work_records: Vec<WorkRecord>,
    pub absence_records: Vec<AbsenceRecord>,
}

impl DayOverview {
    pub fn from_records<'a>(
        date: NaiveDate,
        records: impl IntoIterator<Item = &'a Record>,
    ) -> Self {
        let (work_records, absence_records) = records
            .into_iter()
            .filter(|record| record.date() == date)
            .fold((Vec::new(), Vec::new()), |(mut work_records, mut absence_records), record| {
                match record {
                    Record::Work(work_record) => work_records.push(work_record.clone()),
                    Record::Absence(absence_record) => absence_records.push(absence_record.clone()),
                }

                (work_records, absence_records)
            });

        Self { work_records, absence_records }
    }
    pub fn is_empty(&self) -> bool {
        self.work_records.is_empty() && self.absence_records.is_empty()
    }
}

fn period_time_window(
    period: Period,
    offset: i64,
    today: NaiveDate,
    now: DateTime<Local>,
) -> anyhow::Result<TimeWindow> {
    anyhow::ensure!(offset <= 0, "Timesheet period offset must be zero or negative.");

    let window = match period {
        Period::Week => {
            let current_week_start = reporting::start_of_week(today);
            let weeks_back = offset.unsigned_abs();
            let days_back = weeks_back.checked_mul(7).context("week offset is too large")?;
            let start = current_week_start
                .checked_sub_days(Days::new(days_back))
                .context("week offset is too large")?;
            let end = start.checked_add_days(Days::new(7)).expect("week end should exist");

            date_window(start, end, now)
        }
        Period::Month => {
            let current_month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
                .expect("month start should be valid");
            let months_back =
                u32::try_from(offset.unsigned_abs()).context("month offset is too large")?;
            let start = current_month_start
                .checked_sub_months(Months::new(months_back))
                .context("month offset is too large")?;
            let end =
                start.checked_add_months(Months::new(1)).expect("next month start should be valid");

            date_window(start, end, now)
        }
        Period::Year => {
            let years_back =
                i32::try_from(offset.unsigned_abs()).context("year offset is too large")?;
            let year = today.year().checked_sub(years_back).context("year offset is too large")?;
            let start = NaiveDate::from_ymd_opt(year, 1, 1).expect("year start should be valid");
            let end =
                NaiveDate::from_ymd_opt(year + 1, 1, 1).expect("next year start should be valid");

            date_window(start, end, now)
        }
    };

    Ok(window)
}

fn date_window(start: NaiveDate, end: NaiveDate, now: DateTime<Local>) -> TimeWindow {
    TimeWindow::new(reporting::start_of_day(start), reporting::start_of_day(end).min(now))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::{
        config::ScheduleConfig,
        domain::{
            AbsenceKind,
            AbsencePortion::{self},
            DailyTotals, RecordSet, TimeCategory, WeeklyTotals, WorkContext,
        },
        test_support::*,
    };

    #[derive(Debug, Clone, Copy)]
    struct FixedClock {
        now: DateTime<Local>,
        today: NaiveDate,
    }

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Local> {
            self.now
        }

        fn today(&self) -> NaiveDate {
            self.today
        }
    }

    #[derive(Debug)]
    struct FakeStore {
        records: RecordSet,
    }

    impl Default for FakeStore {
        fn default() -> Self {
            let records = RecordSet::try_from_records(Vec::new()).unwrap();

            Self { records }
        }
    }

    impl FakeStore {
        fn seed(mut self, records: Vec<Record>) -> Self {
            self.records = RecordSet::try_from_records(records).unwrap();

            self
        }
    }

    impl RecordRepository for FakeStore {
        fn records(&self) -> &RecordSet {
            &self.records
        }

        fn commit(&mut self, change: RecordChange) -> anyhow::Result<ChangeOutcome> {
            self.records.apply_change(&change)
        }
    }

    fn default_config_with_schedule(
        mon_minutes: i64,
        tue_minutes: i64,
        wed_minutes: i64,
        thu_minutes: i64,
        fri_minutes: i64,
        sat_minutes: i64,
        sun_minutes: i64,
    ) -> Config {
        Config {
            schedule: ScheduleConfig::new(
                mon_minutes,
                tue_minutes,
                wed_minutes,
                thu_minutes,
                fri_minutes,
                sat_minutes,
                sun_minutes,
            ),
            ..Config::default()
        }
    }

    #[test]
    fn app_uses_injected_clock_timestamp() {
        let fixed_now = Local.with_ymd_and_hms(2026, 1, 10, 9, 30, 0).unwrap();
        let clock = FixedClock { now: fixed_now, today: fixed_now.date_naive() };
        let store = FakeStore::default();
        let mut app = App::new_with_clock(Config::default(), store, clock).unwrap();

        let _ = app.start_work(WorkContext::Office).unwrap();

        let day_overview = app.day(fixed_now.date_naive());
        assert_eq!(day_overview.work_records.len(), 1);
        assert_eq!(*day_overview.work_records[0].timestamp(), fixed_now);
        assert_eq!(
            *day_overview.work_records[0].command(),
            WorkCommand::Start { context: WorkContext::Office }
        );
    }

    #[test]
    fn day_returns_only_requested_date_records() {
        let now = ts(10, 9, 0);

        let store = FakeStore::default().seed(vec![
            work_record(1, 10, 9, 0, WorkCommand::Start { context: WorkContext::Office }),
            work_record(2, 10, 17, 0, WorkCommand::End { title: "Done".into(), description: None }),
            work_record(3, 11, 9, 0, WorkCommand::Start { context: WorkContext::Home }),
            work_record(4, 11, 17, 0, WorkCommand::End { title: "Done".into(), description: None }),
        ]);

        let day1 = ts(10, 0, 0).date_naive();

        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let day_1_overview = app.day(day1);
        assert_eq!(day_1_overview.work_records.len(), 2);
        assert!(
            day_1_overview
                .work_records
                .iter()
                .all(|record| record.timestamp().date_naive() == day1)
        );
    }

    #[test]
    fn time_window_for_historical_range_ends_at_next_midnight() {
        let now = ts(15, 12, 0);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let window =
            app.time_window_for(&DateScope::Range { from: date(10), to: date(12) }).unwrap();

        assert_eq!(window.start(), ts(10, 0, 0));
        assert_eq!(window.end(), ts(13, 0, 0,));
    }

    #[test]
    fn time_window_for_range_including_today_clamps_end_to_now() {
        let now = ts(15, 12, 0);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let window =
            app.time_window_for(&DateScope::Range { from: date(10), to: date(15) }).unwrap();

        assert_eq!(window.start(), ts(10, 0, 0));
        assert_eq!(window.end(), now);
    }

    #[test]
    fn time_window_for_current_week_clamps_end_to_now() {
        let now = ts(15, 12, 0);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let window =
            app.time_window_for(&DateScope::Period { period: Period::Week, offset: 0 }).unwrap();

        assert_eq!(window.start(), ts(12, 0, 0));
        assert_eq!(window.end(), now);
    }

    #[test]
    fn time_window_for_previous_week_uses_previous_calendar_week() {
        let now = ts(15, 12, 0);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let window =
            app.time_window_for(&DateScope::Period { period: Period::Week, offset: -1 }).unwrap();

        assert_eq!(window.start(), ts(5, 0, 0));
        assert_eq!(window.end(), ts(12, 0, 0));
    }

    #[test]
    fn time_window_for_previous_month_uses_previous_calendar_month() {
        let now = ts(15, 12, 0);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let window =
            app.time_window_for(&DateScope::Period { period: Period::Month, offset: -1 }).unwrap();

        assert_eq!(window.start(), Local.with_ymd_and_hms(2025, 12, 1, 0, 0, 0).unwrap());
        assert_eq!(window.end(), Local.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap());
    }

    #[test]
    fn time_window_for_previous_year_uses_previous_calendar_year() {
        let now = ts(15, 12, 0);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let window =
            app.time_window_for(&DateScope::Period { period: Period::Year, offset: -1 }).unwrap();

        assert_eq!(window.start(), Local.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap());
        assert_eq!(window.end(), Local.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap());
    }

    #[test]
    fn timesheet_rejects_positive_period_offset() {
        let now = ts(15, 12, 0);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let err = app
            .time_window_for(&DateScope::Period { period: Period::Week, offset: 1 })
            .unwrap_err();

        assert_eq!(err.to_string(), "Timesheet period offset must be zero or negative.");
    }

    #[test]
    fn timesheet_rejects_reversed_explicit_range() {
        let now = ts(15, 12, 0);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let err = app.timesheet(DateScope::Range { from: date(12), to: date(10) }).unwrap_err();

        assert_eq!(err.to_string(), "Timesheet start date must be on or before end date.");
    }

    #[test]
    fn timesheet_rejects_future_range_dates() {
        let now = ts(15, 12, 0);
        let store = FakeStore::default();
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let err = app.timesheet(DateScope::Range { from: date(16), to: date(17) }).unwrap_err();

        assert_eq!(err.to_string(), "Timesheet start date is in the future.");
    }

    #[test]
    fn range_timesheet_credits_full_day_absence() {
        let now = ts(13, 5, 0);
        let records = vec![absence_record(1, 12, AbsenceKind::Vacation, AbsencePortion::FullDay)];
        let store = FakeStore::default().seed(records);

        let config = default_config_with_schedule(60, 120, 180, 240, 300, 0, 0);

        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let timesheet = app.timesheet(DateScope::Range { from: date(12), to: date(12) }).unwrap();
        assert_eq!(timesheet.weekly_totals(), &[WeeklyTotals::new(date(12), 0, 0, 60, 60)]);
    }

    #[test]
    fn range_timesheet_combines_half_day_work_and_absence() {
        let now = ts(13, 5, 0);
        let records = vec![
            work_record(1, 12, 6, 0, WorkCommand::Start { context: WorkContext::Home }),
            work_record(2, 12, 10, 0, WorkCommand::End { title: "".into(), description: None }),
            absence_record(3, 12, AbsenceKind::Sick, AbsencePortion::HalfDay),
        ];
        let store = FakeStore::default().seed(records);
        let config = default_config_with_schedule(8 * 60, 0, 0, 0, 0, 0, 0);
        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let timesheet = app.timesheet(DateScope::Range { from: date(12), to: date(12) }).unwrap();
        assert_eq!(
            timesheet.weekly_totals(),
            &vec![WeeklyTotals::new(date(12), 4 * 60, 0, 4 * 60, 8 * 60)]
        );
    }

    #[test]
    fn weekly_timesheet_shows_current_day_absence_in_daily_totals() {
        let now = ts(15, 12, 0);
        let records = vec![absence_record(1, 15, AbsenceKind::Holiday, AbsencePortion::FullDay)];
        let store = FakeStore::default().seed(records);
        let config = default_config_with_schedule(0, 0, 0, 8 * 60, 0, 0, 0);
        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let timesheet =
            app.timesheet(DateScope::Period { period: Period::Week, offset: 0 }).unwrap();

        assert!(timesheet.weekly_totals().is_empty());
        assert_eq!(timesheet.daily_totals().len(), 4);
        let expected = vec![
            DailyTotals::new(date(12), 0, 0, 0, 0),
            DailyTotals::new(date(13), 0, 0, 0, 0),
            DailyTotals::new(date(14), 0, 0, 0, 0),
            DailyTotals::new(date(15), 0, 0, 8 * 60, 8 * 60),
        ];
        assert_eq!(timesheet.daily_totals(), &expected);
    }

    #[test]
    fn range_timesheet_weekly_total_credits_absence() {
        let now = ts(15, 12, 0);
        let records = vec![absence_record(1, 12, AbsenceKind::Vacation, AbsencePortion::FullDay)];
        let store = FakeStore::default().seed(records);
        let config = default_config_with_schedule(8 * 60, 0, 0, 0, 0, 0, 0);
        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let timesheet = app.timesheet(DateScope::Range { from: date(12), to: date(14) }).unwrap();

        assert!(timesheet.daily_totals().is_empty());
        assert_eq!(timesheet.weekly_totals().len(), 1);
        let weekly_total = timesheet.weekly_totals().first().unwrap();
        assert_eq!(weekly_total.start(), date(12));
        assert_eq!(weekly_total.target_minutes(), 8 * 60);
        assert_eq!(weekly_total.absence_minutes(), 8 * 60);
        assert_eq!(weekly_total.balance_minutes(), 0);
        assert_eq!(weekly_total.worked_minutes(), 0);
        assert_eq!(weekly_total.break_minutes(), 0);
    }

    #[test]
    fn weekly_timesheet_uses_configured_schedule_and_daily_totals() {
        let now = ts(18, 18, 0);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![
            work_record(1, 12, 8, 0, WorkCommand::Start { context }),
            work_record(2, 12, 9, 0, WorkCommand::End { title: "done".into(), description: None }),
            work_record(3, 13, 8, 0, WorkCommand::Start { context }),
            work_record(4, 13, 10, 0, WorkCommand::End { title: "done".into(), description: None }),
            work_record(5, 14, 8, 0, WorkCommand::Start { context }),
            work_record(6, 14, 11, 0, WorkCommand::End { title: "done".into(), description: None }),
            work_record(7, 15, 8, 0, WorkCommand::Start { context }),
            work_record(8, 15, 12, 0, WorkCommand::End { title: "done".into(), description: None }),
            work_record(9, 16, 8, 0, WorkCommand::Start { context }),
            work_record(
                10,
                16,
                13,
                0,
                WorkCommand::End { title: "done".into(), description: None },
            ),
        ]);
        let config = default_config_with_schedule(60, 120, 180, 240, 300, 0, 0);
        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let timesheet =
            app.timesheet(DateScope::Period { period: Period::Week, offset: 0 }).unwrap();

        assert_eq!(
            timesheet.daily_totals().iter().map(|day| day.worked_minutes()).collect::<Vec<_>>(),
            vec![60, 60 << 1, 3 * 60, 60 << 2, 5 * 60, 0, 0]
        );
        assert!(timesheet.weekly_totals().is_empty());
    }

    #[test]
    fn range_timesheet_with_only_zero_target_days_has_zero_balance() {
        let store = FakeStore::default().seed(vec![]);
        let now = ts(8, 12, 0);
        let config = default_config_with_schedule(60, 2 * 60, 3 * 60, 4 * 60, 5 * 60, 0, 0);
        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let timesheet = app.timesheet(DateScope::Range { from: date(3), to: date(4) }).unwrap();

        assert!(timesheet.daily_totals().is_empty());
        assert_eq!(timesheet.weekly_totals().len(), 1);
    }

    #[test]
    fn weekly_timesheet_uses_targets_through_current_day() {
        let store = FakeStore::default().seed(vec![]);
        let now = ts(8, 12, 0);
        let config = default_config_with_schedule(60, 2 * 60, 3 * 60, 4 * 60, 5 * 60, 0, 0);
        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let timesheet =
            app.timesheet(DateScope::Period { period: Period::Week, offset: 0 }).unwrap();

        assert_eq!(timesheet.daily_totals().len(), 4);
        assert!(timesheet.weekly_totals().is_empty());
    }

    #[test]
    fn weekly_timesheet_excludes_breaks_from_worked_time_and_balance() {
        let context = WorkContext::Home;
        let store = FakeStore::default().seed(vec![
            work_record(1, 8, 8, 0, WorkCommand::Start { context }),
            work_record(2, 8, 12, 0, WorkCommand::BreakStart),
            work_record(3, 8, 13, 0, WorkCommand::BreakEnd { description: None }),
            work_record(4, 8, 18, 0, WorkCommand::End { title: "".into(), description: None }),
        ]);

        let now = ts(8, 20, 0);

        let config = default_config_with_schedule(0, 0, 0, 8 * 60, 0, 0, 0);
        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let timesheet =
            app.timesheet(DateScope::Period { period: Period::Week, offset: 0 }).unwrap();

        let thursday = timesheet.daily_totals().iter().find(|day| day.date() == date(8)).unwrap();
        assert_eq!(thursday.break_minutes(), 60);
    }

    #[test]
    fn month_timesheet_includes_weekly_totals() {
        let now = ts(15, 12, 0);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![
            work_record(1, 5, 8, 0, WorkCommand::Start { context }),
            work_record(2, 5, 16, 0, WorkCommand::End { title: "done".into(), description: None }),
            work_record(3, 12, 8, 0, WorkCommand::Start { context }),
            work_record(4, 12, 16, 0, WorkCommand::End { title: "done".into(), description: None }),
        ]);
        let config = default_config_with_schedule(60 * 8, 60 * 8, 60 * 8, 60 * 8, 60 * 8, 0, 0);
        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let timesheet =
            app.timesheet(DateScope::Period { period: Period::Month, offset: 0 }).unwrap();

        assert_eq!(
            timesheet
                .weekly_totals()
                .iter()
                .map(|week| (
                    week.start(),
                    week.worked_minutes(),
                    week.target_minutes(),
                    week.balance_minutes()
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    // 2026-W01: Schedule has value for Thu (Jan 1) + Fri (Jan 2)
                    Local.with_ymd_and_hms(2025, 12, 29, 0, 0, 0).unwrap().date_naive(),
                    0,
                    16 * 60,
                    -(16 * 60)
                ),
                (
                    // 2026-W02: Schedule has value for Mon-Fri (Jan 5-9)
                    Local.with_ymd_and_hms(2026, 1, 5, 0, 0, 0).unwrap().date_naive(),
                    8 * 60,
                    40 * 60,
                    -(32 * 60)
                ),
                (
                    // 2026-W03: 'Now' is 15 Jan, so Mon-Thu (Jan 12-15)
                    Local.with_ymd_and_hms(2026, 1, 12, 0, 0, 0).unwrap().date_naive(),
                    8 * 60,
                    32 * 60,
                    -(24 * 60)
                ),
            ]
        );
        assert!(timesheet.daily_totals().is_empty());
    }

    #[test]
    fn range_timesheet_uses_targets_for_selected_days() {
        let now = ts(18, 15, 0);
        let store = FakeStore::default();
        let config = default_config_with_schedule(60, 2 * 60, 3 * 60, 4 * 60, 5 * 60, 0, 0);
        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let timesheet = app.timesheet(DateScope::Range { from: date(12), to: date(14) }).unwrap();

        assert!(timesheet.daily_totals().is_empty());
        assert_eq!(timesheet.weekly_totals().len(), 1);
    }

    #[test]
    fn timesheet_for_historical_range_closes_open_day_at_midnight() {
        let now = ts(15, 12, 0);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![work_record(
            1,
            10,
            8,
            0,
            WorkCommand::Start { context },
        )]);
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let timesheet = app.timesheet(DateScope::Range { from: date(10), to: date(10) }).unwrap();

        assert_eq!(
            timesheet.breakdown().active_time(TimeCategory::Work(context)),
            chrono::TimeDelta::hours(16)
        );
        assert_eq!(
            timesheet.breakdown().break_time(TimeCategory::Work(context)),
            chrono::TimeDelta::zero()
        );
    }

    #[test]
    fn timesheet_for_range_uses_records_before_scope_for_carry_in_state() {
        let now = ts(15, 12, 0);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![
            work_record(1, 9, 20, 0, WorkCommand::Start { context }),
            work_record(2, 10, 10, 0, WorkCommand::End { title: "done".into(), description: None }),
        ]);
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let timesheet = app.timesheet(DateScope::Range { from: date(10), to: date(10) }).unwrap();

        assert_eq!(
            timesheet.breakdown().active_time(TimeCategory::Work(context)),
            chrono::TimeDelta::hours(10)
        );
        assert_eq!(
            timesheet.breakdown().break_time(TimeCategory::Work(context)),
            chrono::TimeDelta::zero()
        );
    }

    #[test]
    fn activity_returns_completed_items_for_the_scope() {
        let now = ts(15, 12, 0);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![
            work_record(1, 10, 8, 0, WorkCommand::Start { context }),
            work_record(
                2,
                10,
                10,
                0,
                WorkCommand::Log { title: "handoff".into(), description: None },
            ),
            work_record(3, 10, 12, 0, WorkCommand::End { title: "wrap".into(), description: None }),
        ]);
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let activity_report =
            app.activity(DateScope::Range { from: date(10), to: date(10) }).unwrap();

        assert_eq!(activity_report.rows().len(), 2);
        assert_eq!(activity_report.rows()[0].title(), "handoff");
        assert_eq!(activity_report.rows()[0].duration_minutes(), 120);
        assert_eq!(activity_report.rows()[1].title(), "wrap");
        assert_eq!(activity_report.rows()[1].duration_minutes(), 120);
    }

    #[test]
    fn activity_uses_records_before_scope_for_carry_in_state() {
        let now = ts(15, 12, 0);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![
            work_record(1, 9, 20, 0, WorkCommand::Start { context }),
            work_record(2, 10, 10, 0, WorkCommand::End { title: "done".into(), description: None }),
        ]);
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let activity_report =
            app.activity(DateScope::Range { from: date(10), to: date(10) }).unwrap();

        assert_eq!(activity_report.rows().len(), 1);
        assert_eq!(activity_report.rows()[0].start(), ts(10, 0, 0));
        assert_eq!(activity_report.rows()[0].end(), ts(10, 10, 0));
        assert_eq!(activity_report.rows()[0].duration_minutes(), 10 * 60);
    }

    // Validation tests
    #[test]
    fn start_rejects_invalid_persisted_work_timeline() {
        let now = ts(9, 6, 0);
        let store = FakeStore::default().seed(vec![work_record(
            1,
            8,
            6,
            0,
            WorkCommand::Log { title: "worked harder".to_string(), description: None },
        )]);

        let err = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap_err();

        assert_eq!(err.to_string(), "cannot log while idle for 2026-01-08");
    }

    #[test]
    fn insert_rejects_invalid_work_timeline_mutations() {
        let now = ts(11, 5, 15);
        let store = FakeStore::default().seed(vec![
            work_record(1, 11, 5, 10, WorkCommand::Start { context: WorkContext::Home }),
            work_record(2, 11, 5, 11, WorkCommand::BreakStart),
        ]);

        let mut app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let err = app.log_work("refactoring round  42".into(), None).unwrap_err();

        assert_eq!(err.to_string(), "cannot log while breaking for 2026-01-11");
        assert_eq!(app.store.records().iter().count(), 2);
    }

    #[test]
    fn update_rejects_invalid_work_timeline_permutations() {
        let now = ts(11, 8, 8);
        let store = FakeStore::default().seed(vec![
            work_record(1, 11, 8, 5, WorkCommand::Start { context: WorkContext::Home }),
            work_record(2, 11, 8, 6, WorkCommand::BreakStart),
        ]);

        let mut app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let update =
            RecordInput::new_work(ts(11, 8, 6), WorkCommand::Start { context: WorkContext::Home });

        let err = app.update_record(2.into(), update).unwrap_err();

        assert_eq!(err.to_string(), "cannot start home while already active (home) for 2026-01-11");

        let expected = app.store.records().get(&2.into()).unwrap().as_work().unwrap().command();
        assert!(matches!(expected, WorkCommand::BreakStart));
    }

    #[test]
    fn delete_rejects_invalid_work_timeline_permutations() {
        let now = ts(11, 8, 28);
        let store = FakeStore::default().seed(vec![
            work_record(1, 11, 8, 25, WorkCommand::Start { context: WorkContext::Home }),
            work_record(2, 11, 8, 26, WorkCommand::BreakStart),
        ]);

        let mut app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let err = app.delete_record(1.into()).unwrap_err();

        assert_eq!(err.to_string(), "cannot start break while idle for 2026-01-11");

        let expected = app.store.records().get(&1.into()).unwrap().as_work().unwrap().command();
        assert!(matches!(expected, WorkCommand::Start { context: WorkContext::Home }));
    }

    #[test]
    fn update_rejects_record_type_changes() {
        let now = ts(11, 8, 35);
        let store = FakeStore::default().seed(vec![work_record(
            1,
            11,
            8,
            25,
            WorkCommand::Start { context: WorkContext::Home },
        )]);

        let mut app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let update = RecordInput::Absence(AbsenceRecordInput::new(
            ts(11, 8, 25).date_naive(),
            AbsenceKind::Vacation,
            AbsencePortion::HalfDay,
            Some("".into()),
        ));
        let err = app.update_record(1.into(), update).unwrap_err();

        assert_eq!(err.to_string(), "record 1 cannot be updated with a different record type");

        let expected = app.store.records().get(&1.into()).unwrap().as_work().unwrap().command();
        assert!(matches!(expected, WorkCommand::Start { context: WorkContext::Home }));
    }
}
