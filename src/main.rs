use std::path::PathBuf;

use workaholick::{App, cli};

fn main() -> anyhow::Result<()> {
    // TODO: Handle config file / data dir handling and the name
    let data_dir = "./data-2025.jsonl";

    let app = App::new(PathBuf::from(data_dir));

    cli::run(&app)
}
