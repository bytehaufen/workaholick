mod edit_format;

use chrono::{DateTime, Local, NaiveDate};

use crate::{
    app::edit_format::{parse_edited, render_for_edit},
    config::Config,
    domain::{Command, Entry, EntryId, EntryPatch},
    editor::{Editor, system::FileType},
    store::{EntryStore, ValidatedEntryStore},
};

use anyhow::{Context, Result};

// TODO: Move editor to Cli
#[derive(Debug)]
pub struct App<S: EntryStore, EDITOR: Editor> {
    config: Config,
    v_store: ValidatedEntryStore<S>,
    editor: EDITOR,
}

impl<STORE: EntryStore, EDITOR: Editor> App<STORE, EDITOR> {
    pub fn new(config: Config, store: STORE, editor: EDITOR) -> Result<Self> {
        Ok(Self {
            config,
            v_store: ValidatedEntryStore::new(store)?,
            editor,
        })
    }

    // TODO:
    // - Implement methods: day, status, report
    pub fn append(&mut self, command: Command) -> anyhow::Result<EntryId> {
        let now = now();
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

    pub fn day(&self, _date: NaiveDate) -> Result<()> {
        let entries: Vec<&Entry> = self.v_store.iter().map(|(_, entry)| entry).collect();

        println!("{entries:?}");
        Ok(())
    }

    pub fn edit(&mut self, id: u64) -> Result<()> {
        let entry: EntryPatch = EntryPatch::from_entry(
            self.v_store
                .get(&id.into())
                .with_context(|| format!("find ID {id}"))?,
        );

        let initial = render_for_edit(&entry)?;
        let edited = self.editor.edit(&initial, FileType::Ron)?;
        let patch = parse_edited(&edited)?;
        self.v_store.update(&id.into(), patch)?;

        Ok(())
    }
}

fn now() -> DateTime<Local> {
    Local::now()
}
