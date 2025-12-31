use chrono::{DateTime, Local, NaiveDate};

use crate::{
    cli,
    domain::{Command, Editor, NewEntry, Period},
    store::EntryStore,
};

use anyhow::{Context, Result};

#[derive(Debug, Clone, Default)]
pub struct App<S: EntryStore, E: Editor> {
    store: S,
    editor: E,
}

impl<S: EntryStore, E: Editor> App<S, E> {
    pub fn new(store: S, editor: E) -> Self {
        Self { store, editor }
    }

    // TODO:
    // - Create methods: day, report
    pub fn run(&mut self) -> Result<()> {
        cli::run(self)
    }

    pub fn write(&mut self, command: Command) -> Result<()> {
        let now = Self::now();
        let entry = NewEntry::new(now, command);

        self.store.append(entry)
    }

    pub fn delete(&mut self, id: u64) -> Result<()> {
        if !self.store.delete_by_id(&id.into())? {
            println!("ID {id} does not exist.");
        }

        Ok(())
    }

    pub fn status(&self, _period: Period) -> Result<()> {
        todo!()
    }

    pub fn day(&self, _date: NaiveDate) -> Result<()> {
        let entries = self.store.entries();

        // TODO: Implement
        println!("{entries:#?}");
        Ok(())
    }

    fn now() -> DateTime<Local> {
        Local::now()
    }

    pub fn edit(&mut self, id: u64) -> Result<()> {
        let mut entry: NewEntry = self
            .store
            .find_by_id(id.into())
            .with_context(|| format!("find ID {id}"))?
            .into();

        self.editor.edit(&mut entry)?;

        self.store.update(id.into(), entry)?;

        Ok(())
    }
}
