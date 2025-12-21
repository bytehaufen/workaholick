use std::path::{Path, PathBuf};

use crate::{domain::Command, storage::event_log};

#[derive(Debug, Clone, Default)]
pub struct App {
    data_dir: PathBuf,
}

impl App {
    pub fn new(data_dir: PathBuf) -> Self {
        App { data_dir }
    }

    pub fn handle(&self, command: Command) -> anyhow::Result<()> {
        let path: &Path = &self.data_dir;

        println!("hi there the data dir is: {:?}", path);

        event_log::append(path, &command)
    }
}
