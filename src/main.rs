use anyhow::Result;
use workaholick::{App, JsonlFileStore};

fn main() -> Result<()> {
    // TODO: Handle config file / data dir handling and the name
    let entries_file = "./data/entries.json";

    App::run(JsonlFileStore::new(entries_file)?)
}
