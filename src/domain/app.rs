use std::path::{Path, PathBuf};

use crate::{cli, domain::Command, storage::event_log};

#[derive(Debug, Clone, Default)]
pub struct App {
    data_dir: PathBuf,
}

impl App {
    pub fn run(data_dir: PathBuf) -> anyhow::Result<()> {
        let app = App { data_dir };

        cli::run(&app)
    }

    pub fn handle(&self, command: Command) -> anyhow::Result<()> {
        let path: &Path = &self.data_dir;

        println!("hi there the data dir is: {:?}", path);

        event_log::append(path, &command)
    }
}
