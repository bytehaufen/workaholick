use workaholick::{App, frontend};

fn main() -> anyhow::Result<()> {
    let app = App::new();

    frontend::cli::run(&app)
}
