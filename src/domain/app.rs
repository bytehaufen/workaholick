use chrono::{DateTime, Datelike, Local, NaiveDate};

use crate::{
    cli::{self, Period},
    domain::{Command, NewEntry},
    store::EntryStore,
};

#[derive(Debug, Clone, Default)]
pub struct App<S: EntryStore> {
    store: S,
    // data_dir: PathBuf,
}

impl<S: EntryStore> App<S> {
    // TODO:
    // - Create methods: day, report
    pub fn run(store: S) -> anyhow::Result<()> {
        let mut app = App { store };

        cli::run(&mut app)
    }

    pub fn write(&mut self, command: Command) -> anyhow::Result<()> {
        let now = Self::now();
        let entry = NewEntry::new(now, command);

        self.store.append(&entry)
    }

    pub fn delete(&mut self, id: u64) -> Result<(), anyhow::Error> {
        if !self.store.delete_by_id(&id.into())? {
            println!("ID {id} does not exist.");
        }

        Ok(())
    }

    pub fn status(&self, _period: Period) -> Result<(), anyhow::Error> {
        todo!()
    }

    pub fn day(&self, _date: NaiveDate) -> anyhow::Result<()> {
        let entries = self.store.entries();

        // TODO: Implement
        println!("{entries:#?}");
        Ok(())
    }

    fn now() -> DateTime<Local> {
        Local::now()
    }
    fn this_year() -> i32 {
        Self::now().date_naive().year()
    }
}
