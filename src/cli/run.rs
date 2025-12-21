use super::args::{Cli, CliCommand};
use crate::domain::App;
use crate::web;

use clap::Parser;

pub fn run(app: &App) -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Some(ref config_path) = cli.config {
        println!("Value for config: {}", config_path.display());
    }

    match cli.command {
        CliCommand::Web => web::run(app),
        other => app.handle(other.try_into()?),
    }
}
