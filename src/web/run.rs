use anyhow::Result;

use crate::{EntryReader, EntryWriter, app::App};

pub fn run<S: EntryReader + EntryWriter>(_app: &App<S>) -> Result<()> {
    todo!("Web frontend not implemented yet");
}
