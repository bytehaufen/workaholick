use chrono::{DateTime, Local, NaiveDate};

use crate::{
    config::Config,
    domain::{Command, Entry, EntryId, EntryPatch},
    store::{EntryReader, EntryWriter, ValidatedEntryStore},
};

use anyhow::{Context, Result};

pub trait Clock {
    fn now(&self) -> DateTime<Local>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Local> {
        Local::now()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DayEntryView {
    pub id: EntryId,
    pub timestamp: DateTime<Local>,
    pub command: Command,
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

    // FIXME: Disabled for refactoring
    // pub fn status(&self) -> anyhow::Result<Option<Summary<'_>>> {
    //     let entries = self
    //         .store
    //         .entries()
    //         .iter()
    //         .filter(|entry| *entry.date() == today());
    //
    //     let entries: Vec<&Entry> = entries.collect();
    //
    //     summary(&entries, SummaryRequest::Day(today()))
    // }

    pub fn day(&self, date: NaiveDate) -> Result<Vec<DayEntryView>> {
        if self.v_store.len() == 0 {
            anyhow::bail!("No entries in store.")
        }

        let entries = self
            .v_store
            .iter()
            .filter_map(|entry| (entry.date() == date).then_some(DayEntryView::from(entry)))
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
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow};
    use chrono::TimeZone;

    use super::*;
    use crate::domain::{ActivityKind, WorkContext};

    #[derive(Clone, Copy)]
    struct FixedClock(DateTime<Local>);

    impl Clock for FixedClock {
        fn now(&self) -> DateTime<Local> {
            self.0
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
            todo!()
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
        let clock = FixedClock(fixed_now);
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

        let app = App::new_with_clock(Config::default(), store, FixedClock(day_1_start)).unwrap();

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
}
