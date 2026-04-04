use chrono::{DateTime, Datelike, Days, Local, Months, NaiveDate, TimeZone};

use crate::{
    config::Config,
    domain::{
        Command, Entry, EntryId, EntryPatch, Period, SummaryDay, SummaryReport, SummaryScope,
        SummaryWeek, SummaryWindow, summary,
    },
    store::{EntryReader, EntryWriter, ValidatedEntryStore},
};

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

#[derive(Debug, Clone, PartialEq)]
pub struct DayEntryView {
    pub id: EntryId,
    pub timestamp: DateTime<Local>,
    pub command: Command,
}

impl DayEntryView {
    pub fn date(&self) -> NaiveDate {
        self.timestamp.date_naive()
    }
}

impl From<&Entry> for DayEntryView {
    fn from(entry: &Entry) -> Self {
        Self { id: *entry.id(), timestamp: *entry.timestamp(), command: entry.command().clone() }
    }
}

#[derive(Debug)]
pub struct App<S: EntryReader + EntryWriter, C: Clock = SystemClock> {
    config: Config,
    v_store: ValidatedEntryStore<S>,
    clock: C,
}

impl<STORE: EntryReader + EntryWriter> App<STORE, SystemClock> {
    pub fn new(config: Config, store: STORE) -> Result<Self> {
        Self::new_with_clock(config, store, SystemClock)
    }
}

impl<STORE: EntryReader + EntryWriter, C: Clock> App<STORE, C> {
    pub fn new_with_clock(config: Config, store: STORE, clock: C) -> Result<Self> {
        Ok(Self { config, v_store: ValidatedEntryStore::new(store)?, clock })
    }

    // TODO:
    // - Implement methods: day, status, report
    pub fn append(&mut self, command: Command) -> anyhow::Result<EntryId> {
        let now = self.clock.now();
        let entry = EntryPatch::new(now, command);
        self.v_store.insert(entry)
    }

    pub fn delete(&mut self, id: EntryId) -> Result<Entry> {
        self.v_store.remove(&id)
    }

    pub fn day(&self, date: NaiveDate) -> Result<Vec<DayEntryView>> {
        if self.v_store.len() == 0 {
            anyhow::bail!("No entries in store.")
        }

        let entries = self
            .v_store
            .iter()
            .filter(|entry| entry.date() == date)
            .map(DayEntryView::from)
            .collect();

        Ok(entries)
    }

    pub fn get_entry_patch(&self, id: EntryId) -> Result<EntryPatch> {
        let patch =
            EntryPatch::from_entry(self.v_store.get(&id).with_context(|| format!("find ID {id}"))?);

        Ok(patch)
    }

    pub fn update_entry(&mut self, id: EntryId, patch: EntryPatch) -> Result<()> {
        self.v_store.update(&id, patch)?;

        Ok(())
    }

    pub fn last_date(&self) -> Option<NaiveDate> {
        self.v_store.iter().map(Entry::timestamp).map(DateTime::date_naive).last()
    }

    pub fn summary(&self, scope: SummaryScope) -> anyhow::Result<SummaryReport> {
        let window = self.summary_window(scope.clone())?;
        let entries = self.collect_entries(|entry| *entry.timestamp() < window.end());
        let summary = summary(&entries, window)?;

        match scope {
            SummaryScope::Period { period: Period::Week, .. } => {
                let weekly_days = weekly_summary_days(&entries, window)?;
                Ok(SummaryReport::with_weekly_balance(
                    window,
                    summary,
                    self.config.weekly_target_minutes(),
                    weekly_days,
                ))
            }
            SummaryScope::Period { period: Period::Month | Period::Year, .. } => {
                let weekly_balances = weekly_balance_breakdown(
                    &entries,
                    window,
                    self.config.weekly_target_minutes(),
                )?;
                Ok(SummaryReport::with_weekly_breakdown(window, summary, weekly_balances))
            }
            _ => Ok(SummaryReport::new(window, summary)),
        }
    }

    fn collect_entries<F: Fn(&Entry) -> bool>(&self, predicate: F) -> Vec<DayEntryView> {
        self.v_store.iter().filter(|entry| predicate(entry)).map(DayEntryView::from).collect()
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
        Period::Day => {
            let days_back = offset.unsigned_abs();
            let start =
                today.checked_sub_days(Days::new(days_back)).context("day offset is too large")?;

            date_window(start, next_day(start), now)
        }
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
    entries: &[DayEntryView],
    window: SummaryWindow,
    weekly_target_minutes: i64,
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

        let week_summary = summary(entries, week_window)?;

        Ok(SummaryWeek::new(
            week_start,
            week_summary.total_active_minutes(),
            (week_window.start() == full_week_start && week_window.end() == full_week_end)
                .then_some(weekly_target_minutes),
        ))
    })
    .collect()
}

fn weekly_summary_days(
    entries: &[DayEntryView],
    window: SummaryWindow,
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
                let day_summary = summary(entries, day_window)?;
                Ok(Some(SummaryDay::new(date, day_summary.total_active_minutes())))
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
    use crate::domain::{ActivityKind, WorkContext};

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
        entries: Vec<(EntryId, Entry)>,
        next_id: u64,
    }

    impl FakeStore {
        fn seed(mut self, patches: Vec<(EntryId, EntryPatch)>) -> Self {
            patches.into_iter().for_each(|(id, patch)| {
                self.entries.push((id, Entry::from_patch(id, 1.into(), patch)));
                self.next_id = self.next_id.max(id.value() + 1);
            });
            self.entries.sort_by(|a, b| a.1.sort_key().cmp(&b.1.sort_key()));
            self
        }
    }

    impl EntryReader for FakeStore {
        fn iter(&self) -> Box<dyn Iterator<Item = &Entry> + '_> {
            Box::new(self.entries.iter().map(|(_, entry)| entry))
        }

        fn get(&self, id: &EntryId) -> Option<&Entry> {
            self.entries.iter().find(|(entry_id, _)| entry_id == id).map(|(_, entry)| entry)
        }

        fn len(&self) -> usize {
            self.entries.len()
        }

        fn is_empty(&self) -> bool {
            self.entries.is_empty()
        }
    }

    impl EntryWriter for FakeStore {
        fn next_id(&self) -> EntryId {
            self.next_id.into()
        }

        fn insert(&mut self, entry: EntryPatch) -> Result<EntryId> {
            let id = EntryId::from(self.next_id);
            self.next_id += 1;
            self.entries.push((id, Entry::from_patch(id, 1.into(), entry)));
            self.entries.sort_by(|a, b| a.1.sort_key().cmp(&b.1.sort_key()));
            Ok(id)
        }

        fn remove(&mut self, id: &EntryId) -> Result<Entry> {
            let idx = self
                .entries
                .iter()
                .position(|(entry_id, _)| entry_id == id)
                .ok_or_else(|| anyhow!("id {id} does not exist"))?;
            Ok(self.entries.remove(idx).1)
        }

        fn update(&mut self, id: &EntryId, patch: EntryPatch) -> Result<()> {
            let idx = self
                .entries
                .iter()
                .position(|(entry_id, _)| entry_id == id)
                .ok_or_else(|| anyhow!("id {id} does not exist"))?;
            let version = *self.entries[idx].1.version();
            self.entries[idx].1 = Entry::from_patch(*id, version, patch);
            self.entries.sort_by(|a, b| a.1.sort_key().cmp(&b.1.sort_key()));
            Ok(())
        }
    }

    #[test]
    fn append_uses_injected_clock_timestamp() {
        let fixed_now = Local.with_ymd_and_hms(2026, 1, 10, 9, 30, 0).unwrap();
        let clock = FixedClock { now: fixed_now, today: fixed_now.date_naive() };
        let store = FakeStore::default();
        let mut app = App::new_with_clock(Config::default(), store, clock).unwrap();

        app.append(Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } })
            .unwrap();

        let entries = app.day(fixed_now.date_naive()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timestamp, fixed_now);
        assert_eq!(
            entries[0].command,
            Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } }
        );
    }

    #[test]
    fn day_returns_only_requested_date_entries() {
        let day_1_start = Local.with_ymd_and_hms(2026, 1, 10, 9, 0, 0).unwrap();
        let day_1_end = Local.with_ymd_and_hms(2026, 1, 10, 17, 0, 0).unwrap();
        let day_2_start = Local.with_ymd_and_hms(2026, 1, 11, 9, 0, 0).unwrap();
        let day_2_end = Local.with_ymd_and_hms(2026, 1, 11, 17, 0, 0).unwrap();

        let store = FakeStore::default().seed(vec![
            (
                1.into(),
                EntryPatch::new(
                    day_1_start,
                    Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } },
                ),
            ),
            (
                2.into(),
                EntryPatch::new(
                    day_1_end,
                    Command::End { title: "Done".into(), description: None },
                ),
            ),
            (
                3.into(),
                EntryPatch::new(
                    day_2_start,
                    Command::Start { kind: ActivityKind::Work { context: WorkContext::Home } },
                ),
            ),
            (
                4.into(),
                EntryPatch::new(
                    day_2_end,
                    Command::End { title: "Done".into(), description: None },
                ),
            ),
        ]);

        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now: day_1_start, today: day_1_start.date_naive() },
        )
        .unwrap();

        let day_1_entries = app.day(day_1_start.date_naive()).unwrap();
        assert_eq!(day_1_entries.len(), 2);
        assert!(
            day_1_entries
                .iter()
                .all(|entry| entry.timestamp.date_naive() == day_1_start.date_naive())
        );
    }

    #[test]
    fn day_errors_when_store_is_empty() {
        let store = FakeStore::default();
        let app = App::new(Config::default(), store).unwrap();

        let err = app
            .day(Local.with_ymd_and_hms(2026, 3, 15, 6, 14, 0).unwrap().date_naive())
            .unwrap_err();
        assert_eq!(err.to_string(), "No entries in store.");
    }

    fn ts(day: u32, hour: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 1, day, hour, 0, 0).unwrap()
    }

    fn date(day: u32) -> NaiveDate {
        ts(day, 0).date_naive()
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
    fn summary_window_for_previous_day_uses_previous_day() {
        let now = ts(15, 12);
        let app = App::new_with_clock(
            Config::default(),
            FakeStore::default(),
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let window =
            app.summary_window(SummaryScope::Period { period: Period::Day, offset: -1 }).unwrap();

        assert_eq!(window.start(), ts(14, 0));
        assert_eq!(window.end(), ts(15, 0));
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
            EntryPatch::new(
                ts(10, 9),
                Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } },
            ),
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
    fn weekly_summary_uses_configured_weekly_target_and_day_breakdown() {
        let now = ts(18, 18);
        let kind = ActivityKind::Work { context: WorkContext::Office };
        let store = FakeStore::default().seed(vec![
            (1.into(), EntryPatch::new(ts(12, 8), Command::Start { kind })),
            (
                2.into(),
                EntryPatch::new(
                    ts(12, 16),
                    Command::End { title: "done".into(), description: None },
                ),
            ),
            (3.into(), EntryPatch::new(ts(13, 8), Command::Start { kind })),
            (
                4.into(),
                EntryPatch::new(
                    ts(13, 16),
                    Command::End { title: "done".into(), description: None },
                ),
            ),
            (5.into(), EntryPatch::new(ts(14, 8), Command::Start { kind })),
            (
                6.into(),
                EntryPatch::new(
                    ts(14, 16),
                    Command::End { title: "done".into(), description: None },
                ),
            ),
            (7.into(), EntryPatch::new(ts(15, 8), Command::Start { kind })),
            (
                8.into(),
                EntryPatch::new(
                    ts(15, 16),
                    Command::End { title: "done".into(), description: None },
                ),
            ),
            (9.into(), EntryPatch::new(ts(16, 8), Command::Start { kind })),
            (
                10.into(),
                EntryPatch::new(
                    ts(16, 16),
                    Command::End { title: "done".into(), description: None },
                ),
            ),
        ]);
        let config = Config { weekly_target_hours: 40.0, ..Config::default() };
        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let report = app.summary(SummaryScope::Period { period: Period::Week, offset: 0 }).unwrap();

        assert_eq!(report.weekly_target_minutes(), Some(40 * 60));
        assert_eq!(report.worked_minutes(), 40 * 60);
        assert_eq!(report.balance_minutes(), Some(0));
        assert_eq!(
            report.weekly_days().iter().map(|day| day.active_minutes()).collect::<Vec<_>>(),
            vec![8 * 60, 8 * 60, 8 * 60, 8 * 60, 8 * 60, 0, 0]
        );
        assert!(report.weekly_balances().is_empty());
    }

    #[test]
    fn non_week_summary_does_not_include_weekly_target() {
        let now = ts(12, 12);
        let kind = ActivityKind::Work { context: WorkContext::Office };
        let store = FakeStore::default()
            .seed(vec![(1.into(), EntryPatch::new(ts(12, 8), Command::Start { kind }))]);
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
        let now = Local.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
        let kind = ActivityKind::Work { context: WorkContext::Office };
        let store = FakeStore::default().seed(vec![
            (1.into(), EntryPatch::new(ts(5, 8), Command::Start { kind })),
            (
                2.into(),
                EntryPatch::new(
                    ts(5, 16),
                    Command::End { title: "done".into(), description: None },
                ),
            ),
            (3.into(), EntryPatch::new(ts(12, 8), Command::Start { kind })),
            (
                4.into(),
                EntryPatch::new(
                    ts(12, 16),
                    Command::End { title: "done".into(), description: None },
                ),
            ),
        ]);
        let config = Config { weekly_target_hours: 40.0, ..Config::default() };
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
        let kind = ActivityKind::Work { context: WorkContext::Office };
        let store = FakeStore::default()
            .seed(vec![(1.into(), EntryPatch::new(ts(10, 8), Command::Start { kind }))]);
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let report = app.summary(SummaryScope::Range { from: date(10), to: date(10) }).unwrap();

        assert_eq!(report.summary().active_time(kind), chrono::TimeDelta::hours(16));
        assert_eq!(report.summary().break_time(kind), chrono::TimeDelta::zero());
    }

    #[test]
    fn summary_for_range_uses_entries_before_scope_for_carry_in_state() {
        let now = ts(15, 12);
        let kind = ActivityKind::Work { context: WorkContext::Office };
        let store = FakeStore::default().seed(vec![
            (1.into(), EntryPatch::new(ts(9, 20), Command::Start { kind })),
            (
                2.into(),
                EntryPatch::new(
                    ts(10, 10),
                    Command::End { title: "done".into(), description: None },
                ),
            ),
        ]);
        let app = App::new_with_clock(
            Config::default(),
            store,
            FixedClock { now, today: now.date_naive() },
        )
        .unwrap();

        let report = app.summary(SummaryScope::Range { from: date(10), to: date(10) }).unwrap();

        assert_eq!(report.summary().active_time(kind), chrono::TimeDelta::hours(10));
        assert_eq!(report.summary().break_time(kind), chrono::TimeDelta::zero());
    }
}
