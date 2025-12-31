use anyhow::Result;
use workaholick::{App, JsonlFileStore, ShellEditor};

fn main() -> Result<()> {
    // TODO: Handle config file / data dir handling and the name
    let entries_file = "./data/entries.jsonl";

    App::new(JsonlFileStore::new(entries_file)?, ShellEditor::new()).run()
}
