use chrono::{DateTime, Datelike, Local, NaiveDate};
use std::path::{Path, PathBuf};

use crate::{
    cli,
    domain::{Command, Entry},
    storage,
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
        let now = Self::now();
        let entry = Entry::new(45, 815, now, now.date_naive(), command);
        // FIXME: Continue here:
        // - Calc new id append data

        storage::append(&self.data_dir, &entry)
    }

    pub fn delete(&self, id: u64) -> Result<(), anyhow::Error> {
        let path = Self::log_path_for(Self::this_year(), &self.data_dir)?;

        if !storage::delete_by_id(&path, id)? {
            println!("File {path:?} does not contain ID {id}");
        }

        Ok(())
    }

    fn now() -> DateTime<Local> {
        Local::now()
    }
    fn this_year() -> i32 {
        Self::now().date_naive().year()
    }

    fn log_path_for(year: i32, data_dir: &Path) -> anyhow::Result<PathBuf> {
        storage::events_file(data_dir, year)
    }
}
