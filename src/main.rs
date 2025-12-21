use std::path::PathBuf;

use workaholick::App;

fn main() -> anyhow::Result<()> {
    // TODO: Handle config file / data dir handling and the name
    let data_dir = "./data";

    App::run(PathBuf::from(data_dir))
}
