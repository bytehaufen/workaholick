use anyhow::Result;

use crate::{EntryStore, app::App, editor::Editor};

pub fn run<S: EntryStore, E: Editor>(_app: &App<S, E>) -> Result<()> {
    todo!("Web frontend not implemented yet");
}
