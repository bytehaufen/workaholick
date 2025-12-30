use anyhow::Result;

use crate::{EntryStore, app::App};

pub fn run<S: EntryStore>(_app: &App<S>) -> Result<()> {
    todo!("Web frontend not implemented yet");
}
