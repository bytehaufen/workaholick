use chrono::{DateTime, Datelike, Local, NaiveDate};
use std::path::{Path, PathBuf};

use crate::{
    cli::{self, Period},
    domain::{Command, Entry},
    store,
};

#[derive(Debug, Clone, Default)]
pub struct App {
    data_dir: PathBuf,
}

impl App {
    // TODO:
    // - Create methods: day, report
    pub fn run(data_dir: PathBuf) -> anyhow::Result<()> {
        let app = App { data_dir };

        cli::run(&app)
    }

    pub fn write(&self, command: Command) -> anyhow::Result<()> {
        let path = Self::log_path_for(Self::this_year(), &self.data_dir)?;
        let (max, _entries) = store::read_entries(&path)?;
        let id = max + 1;
        let now = Self::now();
        let entry = Entry::new(id, 1, now, now.date_naive(), command);

        store::append(&path, &entry)
    }

    pub fn delete(&self, id: u64) -> Result<(), anyhow::Error> {
        let path = Self::log_path_for(Self::this_year(), &self.data_dir)?;

        if !store::delete_by_id(&path, id)? {
            println!("File {path:?} does not contain ID {id}");
        }

        Ok(())
    }

    pub fn status(&self, period: Period) -> Result<(), anyhow::Error> {
        todo!()
    }

    pub fn day(&self, date: NaiveDate) -> anyhow::Result<()> {
        let year = date.year();
        let path = Self::log_path_for(year, &self.data_dir)?;
        let (_max, entries) = store::read_entries(&path)?;

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

    fn log_path_for(year: i32, data_dir: &Path) -> anyhow::Result<PathBuf> {
        store::events_file(data_dir, year)
    }
}
