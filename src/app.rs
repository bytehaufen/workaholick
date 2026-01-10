mod edit_format;

use chrono::{DateTime, Local, NaiveDate};

use crate::{
    app::edit_format::{parse_edited, render_for_edit},
    config::Config,
    domain::{Command, Entry, NewEntry, Summary, SummaryRequest, summary},
    editor::{Editor, system::FileType},
    store::EntryStore,
};

use anyhow::{Context, Result, anyhow};

#[derive(Debug, Clone, Default)]
pub struct App<S: EntryStore, E: Editor> {
    config: Config,
    store: S,
    editor: E,
}

impl<S: EntryStore, E: Editor> App<S, E> {
    pub fn new(config: Config, store: S, editor: E) -> Result<Self> {
        Ok(Self {
            config,
            store,
            editor,
        })
    }

    // TODO:
    // - Implement methods: day, status, report
    pub fn write(&mut self, command: Command) -> Result<()> {
        let now = now();
        let entry = NewEntry::new(now, command);

        self.store.append(entry)
    }

    pub fn delete(&mut self, id: u64) -> Result<()> {
        if !self.store.delete_by_id(&id.into())? {
            return Err(anyhow!("ID {id} does not exist."));
        }

        Ok(())
    }

    pub fn status(&self) -> anyhow::Result<Option<Summary<'_>>> {
        let entries = self
            .store
            .entries()
            .iter()
            .filter(|entry| *entry.date() == today());

        let entries: Vec<&Entry> = entries.collect();

        summary(&entries, SummaryRequest::Day(today()))
    }

    pub fn day(&self, _date: NaiveDate) -> Result<()> {
        let entries = self.store.entries();

        println!("{entries:#?}");
        Ok(())
    }

    pub fn edit(&mut self, id: u64) -> Result<()> {
        let entry: NewEntry = self
            .store
            .find_by_id(id.into())
            .with_context(|| format!("find ID {id}"))?
            .into();

        let initial = render_for_edit(&entry)?;
        let edited = self.editor.edit(&initial, FileType::Ron)?;
        let patch = parse_edited(&edited)?;
        self.store.update(id.into(), patch)?;

        Ok(())
    }
}

fn now() -> DateTime<Local> {
    Local::now()
}

fn today() -> NaiveDate {
    now().date_naive()
}
