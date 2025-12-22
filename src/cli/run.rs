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
        CliCommand::In { .. } => app.write(cli.command.try_into()?),
        CliCommand::Out { .. } => app.write(cli.command.try_into()?),
        CliCommand::Log { .. } => app.write(cli.command.try_into()?),
        CliCommand::BreakStart => app.write(cli.command.try_into()?),
        CliCommand::BreakEnd { .. } => app.write(cli.command.try_into()?),
        CliCommand::Day { date } => todo!(),
        CliCommand::Report {} => todo!(),
        CliCommand::Edit {} => todo!(),
        CliCommand::Delete {} => todo!(),
    }
}
