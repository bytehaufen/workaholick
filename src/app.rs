use chrono::{DateTime, Datelike, Days, Local, NaiveDate, TimeZone};

use crate::{
    config::Config,
    domain::{
        Command, Entry, EntryId, EntryPatch, Period, SummaryDay, SummaryReport, SummaryScope,
        SummaryWindow, summary,
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
            SummaryScope::Period(Period::Week) => {
                let weekly_days = weekly_summary_days(&entries, window)?;
                Ok(SummaryReport::with_weekly_balance(
                    window,
                    summary,
                    self.config.weekly_target_minutes(),
                    weekly_days,
                ))
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
            SummaryScope::Period(Period::Today) => SummaryWindow::new(start_of_day(today), now),
            SummaryScope::Period(Period::LastDay) => {
                let date =
                    self.last_date().ok_or_else(|| anyhow::anyhow!("There are no entries."))?;

                day_window(date, now)
            }
            SummaryScope::Period(Period::Week) => {
                let start = today
                    .checked_sub_days(Days::new(today.weekday().num_days_from_monday().into()))
                    .expect("week start should be representable");
                let end = start.checked_add_days(Days::new(7)).expect("week end should exist");

                date_window(start, end, now)
            }
            SummaryScope::Period(Period::Month) => {
                let start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
                    .expect("month start should be valid");
                let (year, month) = if today.month() == 12 {
                    (today.year() + 1, 1)
                } else {
                    (today.year(), today.month() + 1)
                };
                let end = NaiveDate::from_ymd_opt(year, month, 1)
                    .expect("next month start should be valid");

                date_window(start, end, now)
            }
            SummaryScope::Period(Period::Year) => {
                let start = NaiveDate::from_ymd_opt(today.year(), 1, 1)
                    .expect("year start should be valid");
                let end = NaiveDate::from_ymd_opt(today.year() + 1, 1, 1)
                    .expect("next year start should be valid");

                date_window(start, end, now)
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

fn day_window(date: NaiveDate, now: DateTime<Local>) -> SummaryWindow {
    date_window(date, next_day(date), now)
}

fn date_window(start: NaiveDate, end: NaiveDate, now: DateTime<Local>) -> SummaryWindow {
    SummaryWindow::new(start_of_day(start), start_of_day(end).min(now))
}

fn weekly_summary_days(
    entries: &[DayEntryView],
    window: SummaryWindow,
) -> anyhow::Result<Vec<SummaryDay>> {
    let mut date = window.start().date_naive();
    let last_date = window.end().date_naive();
    let mut days = Vec::new();

    while date <= last_date {
        let day_start = start_of_day(date).max(window.start());
        let day_end = start_of_day(next_day(date)).min(window.end());

        if day_end > day_start {
            let day_window = SummaryWindow::new(day_start, day_end);
            let day_summary = summary(entries, day_window)?;
            days.push(SummaryDay::new(date, day_summary.total_active_minutes()));
        }

        date = next_day(date);
    }

    Ok(days)
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
        let mut config = Config::default();
        config.weekly_target_hours = 40.0;
        let app = App::new_with_clock(config, store, FixedClock { now, today: now.date_naive() })
            .unwrap();

        let report = app.summary(SummaryScope::Period(Period::Week)).unwrap();

        assert_eq!(report.weekly_target_minutes(), Some(40 * 60));
        assert_eq!(report.worked_minutes(), 40 * 60);
        assert_eq!(report.balance_minutes(), Some(0));
        assert_eq!(
            report.weekly_days().iter().map(|day| day.active_minutes()).collect::<Vec<_>>(),
            vec![8 * 60, 8 * 60, 8 * 60, 8 * 60, 8 * 60, 0, 0]
        );
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
    }

    #[test]
    fn summary_window_for_last_day_uses_last_logged_date() {
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

        let window = app.summary_window(SummaryScope::Period(Period::LastDay)).unwrap();

        assert_eq!(window.start(), ts(10, 0));
        assert_eq!(window.end(), ts(11, 0));
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
