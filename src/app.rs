use chrono::{DateTime, Datelike, Days, Local, Months, NaiveDate, TimeZone};

use crate::config::Config;
use crate::domain::{
    AbsenceRecord, AbsenceRecordInput, Period, Record, RecordId, RecordInput, SummaryDay,
    SummaryReport, SummaryScope, SummaryWeek, SummaryWindow, WorkCommand, WorkContext,
    WorkItemReport, WorkRecord, WorkRecordInput, report, summary,
};
use crate::store::{RecordReader, RecordWriter, ValidatedRecordStore};

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
pub struct App<S: RecordReader + RecordWriter, C: Clock = SystemClock> {
    config: Config,
    v_store: ValidatedRecordStore<S>,
    clock: C,
}

impl<STORE: RecordReader + RecordWriter> App<STORE, SystemClock> {
    pub fn new(config: Config, store: STORE) -> Result<Self> {
        Self::new_with_clock(config, store, SystemClock)
    }
}

impl<STORE: RecordReader + RecordWriter, C: Clock> App<STORE, C> {
    pub fn new_with_clock(config: Config, store: STORE, clock: C) -> Result<Self> {
        Ok(Self { config, v_store: ValidatedRecordStore::new(store)?, clock })
    }

    ///// Work

    pub fn start_work(&mut self, context: WorkContext) -> anyhow::Result<RecordId> {
        let timestamp = self.clock.now();
        let command = WorkCommand::Start { context };
        self.v_store.insert(RecordInput::Work(WorkRecordInput::new(timestamp, command)))
    }

    pub fn end_work(
        &mut self,
        title: String,
        description: Option<String>,
    ) -> anyhow::Result<RecordId> {
        let timestamp = self.clock.now();
        let command = WorkCommand::End { title, description };
        self.v_store.insert(RecordInput::Work(WorkRecordInput::new(timestamp, command)))
    }

    pub fn log_work(
        &mut self,
        title: String,
        description: Option<String>,
    ) -> anyhow::Result<RecordId> {
        let timestamp = self.clock.now();
        let command = WorkCommand::Log { title, description };
        self.v_store.insert(RecordInput::Work(WorkRecordInput::new(timestamp, command)))
    }

    pub fn start_break(&mut self) -> anyhow::Result<RecordId> {
        let timestamp = self.clock.now();
        let command = WorkCommand::BreakStart;
        self.v_store.insert(RecordInput::Work(WorkRecordInput::new(timestamp, command)))
    }

    pub fn end_break(&mut self, description: Option<String>) -> anyhow::Result<RecordId> {
        let timestamp = self.clock.now();
        let command = WorkCommand::BreakEnd { description };
        self.v_store.insert(RecordInput::Work(WorkRecordInput::new(timestamp, command)))
    }

    ///// Absence
    pub fn add_absence(&mut self, input: AbsenceRecordInput) -> anyhow::Result<RecordId> {
        self.v_store.insert(RecordInput::Absence(input))
    }

    ///// Common
    pub fn get_record(&self, id: RecordId) -> Option<&Record> {
        self.v_store.get(&id)
    }

    pub fn get_record_input(&self, id: RecordId) -> Option<RecordInput> {
        let record = self.v_store.get(&id);

        record.map(RecordInput::from_record)
    }

    pub fn update_record(&mut self, id: RecordId, input: RecordInput) -> anyhow::Result<()> {
        self.v_store.update(&id, input)
    }

    pub fn delete_record(&mut self, id: RecordId) -> anyhow::Result<Record> {
        self.v_store.delete(&id)
    }

    pub fn day(&self, date: NaiveDate) -> DayOverview {
        DayOverview::from_records(date, self.v_store.iter())
    }

    pub fn last_record_date(&self) -> Option<NaiveDate> {
        self.v_store
            .iter()
            .map(|record| match record {
                Record::Work(work_record) => work_record.date(),
                Record::Absence(absence_record) => *absence_record.date(),
            })
            .max()
    }

    pub fn summary(&self, scope: SummaryScope) -> anyhow::Result<SummaryReport> {
        self.build_summary(scope, false)
    }

    pub fn report_summary(&self, scope: SummaryScope) -> anyhow::Result<SummaryReport> {
        self.build_summary(scope, true)
    }

    ///////////// FIXME: old stuff -> need to transform / update / whatever
    fn build_summary(&self, scope: SummaryScope, work_only: bool) -> anyhow::Result<SummaryReport> {
        let window = self.summary_window(scope.clone())?;
        let records = self.v_store.iter().cloned().collect::<Vec<_>>();
        let summary = summary(&records, window)?;
        let summary = if work_only { summary.only_work() } else { summary };

        match scope {
            SummaryScope::Period { period: Period::Week, .. } => {
                let weekly_days = weekly_summary_days(&records, window, work_only)?;
                Ok(SummaryReport::with_weekly_balance(
                    window,
                    summary,
                    self.config.weekly_target_minutes(),
                    weekly_days,
                ))
            }
            SummaryScope::Period { period: Period::Month | Period::Year, .. } => {
                let weekly_balances = weekly_balance_breakdown(
                    &records,
                    window,
                    self.config.weekly_target_minutes(),
                    work_only,
                )?;
                Ok(SummaryReport::with_weekly_breakdown(window, summary, weekly_balances))
            }
            _ => Ok(SummaryReport::new(window, summary)),
        }
    }

    pub fn report(&self, scope: SummaryScope) -> anyhow::Result<WorkItemReport> {
        let window = self.summary_window(scope)?;
        let entries = self.v_store.iter().cloned().collect::<Vec<_>>();

        Ok(report(&entries, window)?)
    }

    fn summary_window(&self, scope: SummaryScope) -> anyhow::Result<SummaryWindow> {
        let now = self.clock.now();
        let today = self.clock.today();

        let window = match scope {
            SummaryScope::Period { period, offset } => {
                period_summary_window(period, offset, today, now)?
            }
            SummaryScope::Range { from, to } => {
                anyhow::ensure!(from <= to, "Summary start date must be on or before end date.");
                let end = next_day(to);

                date_window(from, end, now)
            }
        };

        anyhow::ensure!(!window.is_empty(), "Summary range is empty.");

        Ok(window)
    }
}

pub struct DayOverview {
    pub date: NaiveDate,
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

        Self { date, work_records, absence_records }
    }
    pub fn is_empty(&self) -> bool {
        self.work_records.is_empty() && self.absence_records.is_empty()
    }
}

fn start_of_day(date: NaiveDate) -> DateTime<Local> {
    Local.with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0).unwrap()
}

fn next_day(date: NaiveDate) -> NaiveDate {
    date.succ_opt().expect("summary range end should have a successor day")
}

fn start_of_week(date: NaiveDate) -> NaiveDate {
    date.checked_sub_days(Days::new(date.weekday().num_days_from_monday().into()))
        .expect("week start should be representable")
}

fn period_summary_window(
    period: Period,
    offset: i64,
    today: NaiveDate,
    now: DateTime<Local>,
) -> anyhow::Result<SummaryWindow> {
    anyhow::ensure!(offset <= 0, "Summary period offset must be zero or negative.");

    let window = match period {
        Period::Week => {
            let current_week_start = start_of_week(today);
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

fn date_window(start: NaiveDate, end: NaiveDate, now: DateTime<Local>) -> SummaryWindow {
    SummaryWindow::new(start_of_day(start), start_of_day(end).min(now))
}

fn weekly_balance_breakdown(
    records: &[Record],
    window: SummaryWindow,
    weekly_target_minutes: i64,
    work_only: bool,
) -> anyhow::Result<Vec<SummaryWeek>> {
    let first_week_start = start_of_week(window.start().date_naive());

    std::iter::successors(Some(first_week_start), |week_start| {
        week_start.checked_add_days(Days::new(7))
    })
    .take_while(|&week_start| start_of_day(week_start) < window.end())
    .map(|week_start| {
        let week_end =
            week_start.checked_add_days(Days::new(7)).expect("week end should be representable");

        let full_week_start = start_of_day(week_start);
        let full_week_end = start_of_day(week_end);

        let week_window = SummaryWindow::new(
            full_week_start.max(window.start()),
            full_week_end.min(window.end()),
        );

        let week_summary = summary(records, week_window)?;

        let worked_minutes = if work_only {
            week_summary.work_active_minutes()
        } else {
            week_summary.total_active_minutes()
        };

        Ok(SummaryWeek::new(
            week_start,
            worked_minutes,
            (week_window.start() == full_week_start && week_window.end() == full_week_end)
                .then_some(weekly_target_minutes),
        ))
    })
    .collect()
}

fn weekly_summary_days(
    records: &[Record],
    window: SummaryWindow,
    work_only: bool,
) -> anyhow::Result<Vec<SummaryDay>> {
    let first_date = window.start().date_naive();
    let last_date = window.end().date_naive();

    let days_summary = std::iter::successors(Some(first_date), |date| Some(next_day(*date)))
        .take_while(|date| *date <= last_date)
        .map(|date| -> anyhow::Result<Option<SummaryDay>> {
            let day_start = start_of_day(date).max(window.start());
            let day_end = start_of_day(next_day(date)).min(window.end());

            if day_end > day_start {
                let day_window = SummaryWindow::new(day_start, day_end);
                let day_summary = summary(records, day_window)?;
                let worked_minutes = if work_only {
                    day_summary.work_active_minutes()
                } else {
                    day_summary.total_active_minutes()
                };
                Ok(Some(SummaryDay::new(date, worked_minutes)))
            } else {
                Ok(None)
            }
        })
        .collect::<anyhow::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();

    Ok(days_summary)
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow};
    use chrono::TimeZone;

    use super::*;
    use crate::{
        config::ScheduleConfig,
        domain::{SummaryKind, WorkContext},
    };

    #[derive(Clone, Copy)]
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

    #[derive(Debug, Default)]
    struct FakeStore {
        entries: Vec<(RecordId, Record)>,
        next_id: u64,
    }

    impl FakeStore {
        fn seed(mut self, inputs: Vec<(RecordId, RecordInput)>) -> Self {
            inputs.into_iter().for_each(|(id, input)| {
                self.entries.push((id, Record::from_input(id, input)));
                self.next_id = self.next_id.max(id.value() + 1);
            });
            self.entries.sort_by(|a, b| a.1.sort_key().cmp(&b.1.sort_key()));
            self
        }
    }

    impl RecordReader for FakeStore {
        fn iter(&self) -> Box<dyn Iterator<Item = &Record> + '_> {
            Box::new(self.entries.iter().map(|(_, record)| record))
        }

        fn get(&self, id: &RecordId) -> Option<&Record> {
            self.entries.iter().find(|(record_id, _)| record_id == id).map(|(_, record)| record)
        }

        fn len(&self) -> usize {
            self.entries.len()
        }

        fn is_empty(&self) -> bool {
            self.entries.is_empty()
        }
    }

    impl RecordWriter for FakeStore {
        fn next_id(&self) -> RecordId {
            self.next_id.into()
        }

        fn insert(&mut self, input: RecordInput) -> Result<RecordId> {
            let id = RecordId::from(self.next_id);
            self.next_id += 1;
            self.entries.push((id, Record::from_input(id, input)));
            self.entries.sort_by(|a, b| a.1.sort_key().cmp(&b.1.sort_key()));
            Ok(id)
        }

        fn delete(&mut self, id: &RecordId) -> Result<Record> {
            let idx = self
                .entries
                .iter()
                .position(|(entry_id, _)| entry_id == id)
                .ok_or_else(|| anyhow!("id {id} does not exist"))?;
            Ok(self.entries.remove(idx).1)
        }

        fn update(&mut self, id: &RecordId, input: RecordInput) -> Result<()> {
            let idx = self
                .entries
                .iter()
                .position(|(entry_id, _)| entry_id == id)
                .ok_or_else(|| anyhow!("id {id} does not exist"))?;
            self.entries[idx].1 = Record::from_input(*id, input);
            self.entries.sort_by(|a, b| a.1.sort_key().cmp(&b.1.sort_key()));
            Ok(())
        }
    }

    fn ts(day: u32, hour: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 1, day, hour, 0, 0).unwrap()
    }

    fn date(day: u32) -> NaiveDate {
        ts(day, 0).date_naive()
    }

    #[test]
    fn app_uses_injected_clock_timestamp() {
        let fixed_now = Local.with_ymd_and_hms(2026, 1, 10, 9, 30, 0).unwrap();
        let clock = FixedClock { now: fixed_now, today: fixed_now.date_naive() };
        let store = FakeStore::default();
        let mut app = App::new_with_clock(Config::default(), store, clock).unwrap();

        app.start_work(WorkContext::Office).unwrap();

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
                RecordInput::Work(WorkRecordInput::new(
                    day_1_start,
                    WorkCommand::Start { context: WorkContext::Office },
                )),
            ),
            (
                2.into(),
                RecordInput::Work(WorkRecordInput::new(
                    day_1_end,
                    WorkCommand::End { title: "Done".into(), description: None },
                )),
            ),
            (
                3.into(),
                RecordInput::Work(WorkRecordInput::new(
                    day_2_start,
                    WorkCommand::Start { context: WorkContext::Home },
                )),
            ),
            (
                4.into(),
                RecordInput::Work(WorkRecordInput::new(
                    day_2_end,
                    WorkCommand::End { title: "Done".into(), description: None },
                )),
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
    fn summary_window_for_historical_range_ends_at_next_midnight() {
        let now = ts(15, 12);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let window =
            app.summary_window(SummaryScope::Range { from: date(10), to: date(12) }).unwrap();

        assert_eq!(window.start(), ts(10, 0));
        assert_eq!(window.end(), ts(13, 0));
    }

    #[test]
    fn summary_window_for_range_including_today_clamps_end_to_now() {
        let now = ts(15, 12);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let window =
            app.summary_window(SummaryScope::Range { from: date(10), to: date(15) }).unwrap();

        assert_eq!(window.start(), ts(10, 0));
        assert_eq!(window.end(), now);
    }

    #[test]
    fn summary_window_for_current_week_clamps_end_to_now() {
        let now = ts(15, 12);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let window =
            app.summary_window(SummaryScope::Period { period: Period::Week, offset: 0 }).unwrap();

        assert_eq!(window.start(), ts(12, 0));
        assert_eq!(window.end(), now);
    }

    #[test]
    fn summary_window_for_previous_week_uses_previous_calendar_week() {
        let now = ts(15, 12);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let window =
            app.summary_window(SummaryScope::Period { period: Period::Week, offset: -1 }).unwrap();

        assert_eq!(window.start(), ts(5, 0));
        assert_eq!(window.end(), ts(12, 0));
    }

    #[test]
    fn summary_window_for_previous_month_uses_previous_calendar_month() {
        let now = ts(15, 12);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let window =
            app.summary_window(SummaryScope::Period { period: Period::Month, offset: -1 }).unwrap();

        assert_eq!(window.start(), Local.with_ymd_and_hms(2025, 12, 1, 0, 0, 0).unwrap());
        assert_eq!(window.end(), Local.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap());
    }

    #[test]
    fn summary_window_for_previous_year_uses_previous_calendar_year() {
        let now = ts(15, 12);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let window =
            app.summary_window(SummaryScope::Period { period: Period::Year, offset: -1 }).unwrap();

        assert_eq!(window.start(), Local.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap());
        assert_eq!(window.end(), Local.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap());
    }

    #[test]
    fn summary_rejects_positive_period_offset() {
        let now = ts(15, 12);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let err = app
            .summary_window(SummaryScope::Period { period: Period::Week, offset: 1 })
            .unwrap_err();

        assert_eq!(err.to_string(), "Summary period offset must be zero or negative.");
    }

    #[test]
    fn summary_rejects_reversed_explicit_range() {
        let now = ts(15, 12);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let err = app.summary(SummaryScope::Range { from: date(12), to: date(10) }).unwrap_err();

        assert_eq!(err.to_string(), "Summary start date must be on or before end date.");
    }

    #[test]
    fn summary_rejects_future_explicit_range_without_overlap() {
        let now = ts(15, 12);
        let store = FakeStore::default().seed(vec![(
            1.into(),
            RecordInput::Work(WorkRecordInput::new(
                ts(10, 9),
                WorkCommand::Start { context: WorkContext::Office },
            )),
        )]);
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let err = app.summary(SummaryScope::Range { from: date(16), to: date(17) }).unwrap_err();

        assert_eq!(err.to_string(), "Summary range is empty.");
    }

    #[test]
    fn weekly_summary_uses_configured_schedule_and_day_breakdown() {
        let now = ts(18, 18);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![
            (
                1.into(),
                RecordInput::Work(WorkRecordInput::new(ts(12, 8), WorkCommand::Start { context })),
            ),
            (
                2.into(),
                RecordInput::Work(WorkRecordInput::new(
                    ts(12, 9),
                    WorkCommand::End { title: "done".into(), description: None },
                )),
            ),
            (
                3.into(),
                RecordInput::Work(WorkRecordInput::new(ts(13, 8), WorkCommand::Start { context })),
            ),
            (
                4.into(),
                RecordInput::Work(WorkRecordInput::new(
                    ts(13, 10),
                    WorkCommand::End { title: "done".into(), description: None },
                )),
            ),
            (
                5.into(),
                RecordInput::Work(WorkRecordInput::new(ts(14, 8), WorkCommand::Start { context })),
            ),
            (
                6.into(),
                RecordInput::Work(WorkRecordInput::new(
                    ts(14, 11),
                    WorkCommand::End { title: "done".into(), description: None },
                )),
            ),
            (
                7.into(),
                RecordInput::Work(WorkRecordInput::new(ts(15, 8), WorkCommand::Start { context })),
            ),
            (
                8.into(),
                RecordInput::Work(WorkRecordInput::new(
                    ts(15, 12),
                    WorkCommand::End { title: "done".into(), description: None },
                )),
            ),
            (
                9.into(),
                RecordInput::Work(WorkRecordInput::new(ts(16, 8), WorkCommand::Start { context })),
            ),
            (
                10.into(),
                RecordInput::Work(WorkRecordInput::new(
                    ts(16, 13),
                    WorkCommand::End { title: "done".into(), description: None },
                )),
            ),
        ]);
        let config = Config {
            schedule: ScheduleConfig::new(60, 120, 180, 240, 300, 0, 0),
            ..Config::default()
        };
        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let report = app.summary(SummaryScope::Period { period: Period::Week, offset: 0 }).unwrap();

        assert_eq!(report.weekly_target_minutes(), Some(1 + 2 + 3 + 4 + 5).map(|v| v * 60));
        assert_eq!(report.worked_minutes(), (1 + 2 + 3 + 4 + 5) * 60);
        assert_eq!(report.balance_minutes(), Some(0));
        assert_eq!(
            report.weekly_days().iter().map(|day| day.active_minutes()).collect::<Vec<_>>(),
            vec![60, 60 << 1, 3 * 60, 60 << 2, 5 * 60, 0, 0]
        );
        assert!(report.weekly_balances().is_empty());
    }

    #[test]
    fn non_week_summary_does_not_include_weekly_target() {
        let now = ts(12, 12);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![(
            1.into(),
            RecordInput::Work(WorkRecordInput::new(ts(12, 8), WorkCommand::Start { context })),
        )]);
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let report = app.summary(SummaryScope::Range { from: date(12), to: date(12) }).unwrap();

        assert_eq!(report.weekly_target_minutes(), None);
        assert_eq!(report.worked_minutes(), 4 * 60);
        assert_eq!(report.balance_minutes(), None);
        assert!(report.weekly_days().is_empty());
        assert!(report.weekly_balances().is_empty());
    }

    #[test]
    fn month_summary_includes_weekly_balance_breakdown() {
        let now = ts(15, 12);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![
            (
                1.into(),
                RecordInput::Work(WorkRecordInput::new(ts(5, 8), WorkCommand::Start { context })),
            ),
            (
                2.into(),
                RecordInput::Work(WorkRecordInput::new(
                    ts(5, 16),
                    WorkCommand::End { title: "done".into(), description: None },
                )),
            ),
            (
                3.into(),
                RecordInput::Work(WorkRecordInput::new(ts(12, 8), WorkCommand::Start { context })),
            ),
            (
                4.into(),
                RecordInput::Work(WorkRecordInput::new(
                    ts(12, 16),
                    WorkCommand::End { title: "done".into(), description: None },
                )),
            ),
        ]);
        let config = Config {
            schedule: ScheduleConfig::new(60 * 8, 60 * 8, 60 * 8, 60 * 8, 60 * 8, 0, 0),
            ..Config::default()
        };
        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let report =
            app.summary(SummaryScope::Period { period: Period::Month, offset: 0 }).unwrap();

        assert_eq!(
            report
                .weekly_balances()
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
        assert!(report.weekly_days().is_empty());
        assert_eq!(report.weekly_target_minutes(), None);
    }

    #[test]
    fn summary_for_historical_range_closes_open_day_at_midnight() {
        let now = ts(15, 12);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![(
            1.into(),
            RecordInput::Work(WorkRecordInput::new(ts(10, 8), WorkCommand::Start { context })),
        )]);
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let report = app.summary(SummaryScope::Range { from: date(10), to: date(10) }).unwrap();

        assert_eq!(
            report.summary().active_time(SummaryKind::Work(context)),
            chrono::TimeDelta::hours(16)
        );
        assert_eq!(
            report.summary().break_time(SummaryKind::Work(context)),
            chrono::TimeDelta::zero()
        );
    }

    #[test]
    fn summary_for_range_uses_entries_before_scope_for_carry_in_state() {
        let now = ts(15, 12);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![
            (
                1.into(),
                RecordInput::Work(WorkRecordInput::new(ts(9, 20), WorkCommand::Start { context })),
            ),
            (
                2.into(),
                RecordInput::Work(WorkRecordInput::new(
                    ts(10, 10),
                    WorkCommand::End { title: "done".into(), description: None },
                )),
            ),
        ]);
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let report = app.summary(SummaryScope::Range { from: date(10), to: date(10) }).unwrap();

        assert_eq!(
            report.summary().active_time(SummaryKind::Work(context)),
            chrono::TimeDelta::hours(10)
        );
        assert_eq!(
            report.summary().break_time(SummaryKind::Work(context)),
            chrono::TimeDelta::zero()
        );
    }

    #[test]
    fn report_returns_completed_items_for_the_scope() {
        let now = ts(15, 12);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![
            (
                1.into(),
                RecordInput::Work(WorkRecordInput::new(ts(10, 8), WorkCommand::Start { context })),
            ),
            (
                2.into(),
                RecordInput::Work(WorkRecordInput::new(
                    Local.with_ymd_and_hms(2026, 1, 10, 10, 0, 0).unwrap(),
                    WorkCommand::Log { title: "handoff".into(), description: None },
                )),
            ),
            (
                3.into(),
                RecordInput::Work(WorkRecordInput::new(
                    ts(10, 12),
                    WorkCommand::End { title: "wrap".into(), description: None },
                )),
            ),
        ]);
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let report = app.report(SummaryScope::Range { from: date(10), to: date(10) }).unwrap();

        assert_eq!(report.rows().len(), 2);
        assert_eq!(report.rows()[0].title(), "handoff");
        assert_eq!(report.rows()[0].duration_minutes(), 120);
        assert_eq!(report.rows()[1].title(), "wrap");
        assert_eq!(report.rows()[1].duration_minutes(), 120);
    }

    #[test]
    fn report_uses_entries_before_scope_for_carry_in_state() {
        let now = ts(15, 12);
        let context = WorkContext::Office;
        let store = FakeStore::default().seed(vec![
            (
                1.into(),
                RecordInput::Work(WorkRecordInput::new(
                    Local.with_ymd_and_hms(2026, 1, 9, 20, 0, 0).unwrap(),
                    WorkCommand::Start { context },
                )),
            ),
            (
                2.into(),
                RecordInput::Work(WorkRecordInput::new(
                    ts(10, 10),
                    WorkCommand::End { title: "done".into(), description: None },
                )),
            ),
        ]);
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let report = app.report(SummaryScope::Range { from: date(10), to: date(10) }).unwrap();

        assert_eq!(report.rows().len(), 1);
        assert_eq!(report.rows()[0].start(), ts(10, 0));
        assert_eq!(report.rows()[0].end(), ts(10, 10));
        assert_eq!(report.rows()[0].duration_minutes(), 10 * 60);
    }
}
