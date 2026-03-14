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

    pub fn delete(&mut self, id: u64) -> Result<Entry> {
        self.v_store.remove(&id.into())
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

    pub fn day(&self, date: NaiveDate) -> Result<Vec<Entry>> {
        let entries = self
            .v_store
            .iter()
            .filter_map(|(_, entry)| (entry.date() == date).then_some(entry.clone()))
            .collect();

        Ok(entries)
    }

    pub fn get_entry_patch(&self, id: u64) -> Result<EntryPatch> {
        let patch = EntryPatch::from_entry(
            self.v_store.get(&id.into()).with_context(|| format!("find ID {id}"))?,
        );

        Ok(patch)
    }

    pub fn update_entry(&mut self, id: u64, patch: EntryPatch) -> Result<()> {
        self.v_store.update(&id.into(), patch)?;

        Ok(())
    }
}
