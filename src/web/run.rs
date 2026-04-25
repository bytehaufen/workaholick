use anyhow::Result;

use crate::{RecordReader, RecordWriter, app::App};

pub fn run<S: RecordReader + RecordWriter>(_app: &App<S>) -> Result<()> {
    todo!("Web frontend not implemented yet");
}
