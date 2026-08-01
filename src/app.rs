use chrono::{DateTime, Datelike, Days, Local, Months, NaiveDate};

use crate::config::Config;
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
    config: Config,
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

        Ok(Self { config, store, clock })
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
        self.build_timesheet(scope)
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

    ///////////// FIXME: old stuff -> need to transform / update / whatever
    fn build_timesheet(&self, scope: DateScope) -> anyhow::Result<Timesheet> {
        let window = self.time_window_for(&scope)?;
        let records = self.store.records().iter().cloned().collect::<Vec<_>>();

        build_timesheet(&records, window, &scope, self.config.weekly_target_minutes())
    }

    pub fn activity(&self, scope: DateScope) -> anyhow::Result<ActivityReport> {
        let window = self.time_window_for(&scope)?;
        let records = self.store.records().iter().cloned().collect::<Vec<_>>();

        Ok(activity_report::calculate_activity_report(&records, window)?)
    }

    fn time_window_for(&self, scope: &DateScope) -> anyhow::Result<TimeWindow> {
        let now = self.clock.now();
        let today = self.clock.today();

        let window = match scope {
            DateScope::Period { period, offset } => {
                period_time_window(*period, *offset, today, now)?
            }
            DateScope::Range { from, to } => {
                anyhow::ensure!(from <= to, "Timesheet start date must be on or before end date.");
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
        domain::{AbsenceKind, AbsencePortion::HalfDay, RecordSet, TimeCategory, WorkContext},
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
        fn seed(mut self, inputs: Vec<(RecordId, RecordInput)>) -> Self {
            let records =
                inputs.into_iter().map(|(id, input)| Record::from_input(id, input)).collect();
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
        let day_1_start = Local.with_ymd_and_hms(2026, 1, 10, 9, 0, 0).unwrap();
        let day_1_end = Local.with_ymd_and_hms(2026, 1, 10, 17, 0, 0).unwrap();
        let day_2_start = Local.with_ymd_and_hms(2026, 1, 11, 9, 0, 0).unwrap();
        let day_2_end = Local.with_ymd_and_hms(2026, 1, 11, 17, 0, 0).unwrap();

        let store = FakeStore::default().seed(vec![
            (
                1.into(),
                RecordInput::new_work(
                    day_1_start,
                    WorkCommand::Start { context: WorkContext::Office },
                ),
            ),
            (
                2.into(),
                RecordInput::new_work(
                    day_1_end,
                    WorkCommand::End { title: "Done".into(), description: None },
                ),
            ),
            (
                3.into(),
                RecordInput::new_work(
                    day_2_start,
                    WorkCommand::Start { context: WorkContext::Home },
                ),
            ),
            (
                4.into(),
                RecordInput::new_work(
                    day_2_end,
                    WorkCommand::End { title: "Done".into(), description: None },
                ),
            ),
        ]);

        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now: day_1_start, today: day_1_start.date_naive() },
        )
        .unwrap();

        let day_1_overview = app.day(day_1_start.date_naive());
        assert_eq!(day_1_overview.work_records.len(), 2);
        assert!(
            day_1_overview
                .work_records
                .iter()
                .all(|record| record.timestamp().date_naive() == day_1_start.date_naive())
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
    fn timesheet_rejects_future_explicit_range_without_overlap() {
        let now = ts(15, 12, 0);
        let store = FakeStore::default().seed(vec![(
            1.into(),
            RecordInput::new_work(
                ts(10, 9, 0),
                WorkCommand::Start { context: WorkContext::Office },
            ),
        )]);
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let err = app.timesheet(DateScope::Range { from: date(16), to: date(17) }).unwrap_err();

        assert_eq!(err.to_string(), "Timesheet range is empty.");
    }

    #[test]
    fn weekly_timesheet_uses_configured_schedule_and_daily_totals() {
        let now = ts(18, 18, 0);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![
            (1.into(), RecordInput::new_work(ts(12, 8, 0), WorkCommand::Start { context })),
            (
                2.into(),
                RecordInput::new_work(
                    ts(12, 9, 0),
                    WorkCommand::End { title: "done".into(), description: None },
                ),
            ),
            (3.into(), RecordInput::new_work(ts(13, 8, 0), WorkCommand::Start { context })),
            (
                4.into(),
                RecordInput::new_work(
                    ts(13, 10, 0),
                    WorkCommand::End { title: "done".into(), description: None },
                ),
            ),
            (5.into(), RecordInput::new_work(ts(14, 8, 0), WorkCommand::Start { context })),
            (
                6.into(),
                RecordInput::new_work(
                    ts(14, 11, 0),
                    WorkCommand::End { title: "done".into(), description: None },
                ),
            ),
            (7.into(), RecordInput::new_work(ts(15, 8, 0), WorkCommand::Start { context })),
            (
                8.into(),
                RecordInput::new_work(
                    ts(15, 12, 0),
                    WorkCommand::End { title: "done".into(), description: None },
                ),
            ),
            (9.into(), RecordInput::new_work(ts(16, 8, 0), WorkCommand::Start { context })),
            (
                10.into(),
                RecordInput::new_work(
                    ts(16, 13, 0),
                    WorkCommand::End { title: "done".into(), description: None },
                ),
            ),
        ]);
        let config = Config {
            schedule: ScheduleConfig::new(60, 120, 180, 240, 300, 0, 0),
            ..Config::default()
        };
        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let timesheet =
            app.timesheet(DateScope::Period { period: Period::Week, offset: 0 }).unwrap();

        assert_eq!(timesheet.weekly_target_minutes(), Some(1 + 2 + 3 + 4 + 5).map(|v| v * 60));
        assert_eq!(timesheet.worked_minutes(), (1 + 2 + 3 + 4 + 5) * 60);
        assert_eq!(timesheet.balance_minutes(), Some(0));
        assert_eq!(
            timesheet.daily_totals().iter().map(|day| day.worked_minutes()).collect::<Vec<_>>(),
            vec![60, 60 << 1, 3 * 60, 60 << 2, 5 * 60, 0, 0]
        );
        assert!(timesheet.weekly_totals().is_empty());
    }

    #[test]
    fn non_week_timesheet_does_not_include_weekly_target() {
        let now = ts(12, 12, 0);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![(
            1.into(),
            RecordInput::new_work(ts(12, 8, 0), WorkCommand::Start { context }),
        )]);
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let timesheet = app.timesheet(DateScope::Range { from: date(12), to: date(12) }).unwrap();

        assert_eq!(timesheet.weekly_target_minutes(), None);
        assert_eq!(timesheet.worked_minutes(), 4 * 60);
        assert_eq!(timesheet.balance_minutes(), None);
        assert!(timesheet.daily_totals().is_empty());
        assert!(timesheet.weekly_totals().is_empty());
    }

    #[test]
    fn month_timesheet_includes_weekly_totals() {
        let now = ts(15, 12, 0);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![
            (1.into(), RecordInput::new_work(ts(5, 8, 0), WorkCommand::Start { context })),
            (
                2.into(),
                RecordInput::new_work(
                    ts(5, 16, 0),
                    WorkCommand::End { title: "done".into(), description: None },
                ),
            ),
            (3.into(), RecordInput::new_work(ts(12, 8, 0), WorkCommand::Start { context })),
            (
                4.into(),
                RecordInput::new_work(
                    ts(12, 16, 0),
                    WorkCommand::End { title: "done".into(), description: None },
                ),
            ),
        ]);
        let config = Config {
            schedule: ScheduleConfig::new(60 * 8, 60 * 8, 60 * 8, 60 * 8, 60 * 8, 0, 0),
            ..Config::default()
        };
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
                    Local.with_ymd_and_hms(2025, 12, 29, 0, 0, 0).unwrap().date_naive(),
                    0,
                    None,
                    None
                ),
                (
                    Local.with_ymd_and_hms(2026, 1, 5, 0, 0, 0).unwrap().date_naive(),
                    8 * 60,
                    Some(40 * 60),
                    Some(-(32 * 60))
                ),
                (
                    Local.with_ymd_and_hms(2026, 1, 12, 0, 0, 0).unwrap().date_naive(),
                    8 * 60,
                    None,
                    None
                ),
            ]
        );
        assert!(timesheet.daily_totals().is_empty());
        assert_eq!(timesheet.weekly_target_minutes(), None);
    }

    #[test]
    fn timesheet_for_historical_range_closes_open_day_at_midnight() {
        let now = ts(15, 12, 0);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![(
            1.into(),
            RecordInput::new_work(ts(10, 8, 0), WorkCommand::Start { context }),
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
            (1.into(), RecordInput::new_work(ts(9, 20, 0), WorkCommand::Start { context })),
            (
                2.into(),
                RecordInput::new_work(
                    ts(10, 10, 0),
                    WorkCommand::End { title: "done".into(), description: None },
                ),
            ),
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
            (1.into(), RecordInput::new_work(ts(10, 8, 0), WorkCommand::Start { context })),
            (
                2.into(),
                RecordInput::new_work(
                    Local.with_ymd_and_hms(2026, 1, 10, 10, 0, 0).unwrap(),
                    WorkCommand::Log { title: "handoff".into(), description: None },
                ),
            ),
            (
                3.into(),
                RecordInput::new_work(
                    ts(10, 12, 0),
                    WorkCommand::End { title: "wrap".into(), description: None },
                ),
            ),
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
            (
                1.into(),
                RecordInput::new_work(
                    Local.with_ymd_and_hms(2026, 1, 9, 20, 0, 0).unwrap(),
                    WorkCommand::Start { context },
                ),
            ),
            (
                2.into(),
                RecordInput::new_work(
                    ts(10, 10, 0),
                    WorkCommand::End { title: "done".into(), description: None },
                ),
            ),
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
        let store = FakeStore::default().seed(vec![(
            1.into(),
            RecordInput::new_work(
                ts(8, 6, 0),
                WorkCommand::Log { title: "worked harder".to_string(), description: None },
            ),
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
            make_record_entry(1, 11, 5, 10, WorkCommand::Start { context: WorkContext::Home }),
            make_record_entry(2, 11, 5, 11, WorkCommand::BreakStart),
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
            make_record_entry(1, 11, 8, 5, WorkCommand::Start { context: WorkContext::Home }),
            make_record_entry(2, 11, 8, 6, WorkCommand::BreakStart),
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
            make_record_entry(1, 11, 8, 25, WorkCommand::Start { context: WorkContext::Home }),
            make_record_entry(2, 11, 8, 26, WorkCommand::BreakStart),
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
        let store = FakeStore::default().seed(vec![make_record_entry(
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
            HalfDay,
            Some("".into()),
        ));
        let err = app.update_record(1.into(), update).unwrap_err();

        assert_eq!(err.to_string(), "record 1 cannot be updated with a different record type");

        let expected = app.store.records().get(&1.into()).unwrap().as_work().unwrap().command();
        assert!(matches!(expected, WorkCommand::Start { context: WorkContext::Home }));
    }
}
