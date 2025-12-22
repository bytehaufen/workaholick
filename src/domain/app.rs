use chrono::{Datelike, Local};
use std::path::PathBuf;

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
    pub fn run(data_dir: PathBuf) -> anyhow::Result<()> {
        let app = App { data_dir };

        cli::run(&app)
    }

    pub fn write(&self, command: Command) -> anyhow::Result<()> {
        let path = &self.data_dir;

        let timestamp = Local::now();
        let date = timestamp.date_naive();
        let year = date.year();

        let file = storage::events_file(&self.data_dir, year);

        // TODO:
        // - Rename this method to write
        // - Create methods: day, report

        let entry = Entry::new(42, 815, timestamp, date, command);

        storage::append(path, &entry)
    }
}
