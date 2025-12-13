mod core;
mod frontend;

fn main() -> anyhow::Result<()> {
    let app = core::App::new();

    frontend::cli::run(&app)
}
