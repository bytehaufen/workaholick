use workaholick::{App, JsonlFileStore};

fn main() -> anyhow::Result<()> {
    // TODO: Handle config file / data dir handling and the name
    let data_dir = "./data";

    App::run(JsonlFileStore::new(data_dir.into()))
}
