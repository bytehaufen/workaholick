use workaholick::{App, cli};

fn main() -> anyhow::Result<()> {
    let app = App::new();

    cli::run(&app)
}
