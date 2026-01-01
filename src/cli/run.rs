use super::args::{Cli, CliCommand};
use crate::App;
use crate::editor::Editor;
use crate::{EntryStore, web};

use anyhow::Result;
use clap::Parser;

pub fn run<S: EntryStore, E: Editor>(app: &mut App<S, E>) -> Result<()> {
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
        CliCommand::Delete { id } => app.delete(id),
        CliCommand::Edit { id } => app.edit(id),
        CliCommand::Status { period } => app.status(period.into()),
        CliCommand::Day { date } => app.day(date),
        CliCommand::Report {} => todo!(),
        // TODO: Next steps:
        // - Make a clear interface for status stuff in app, and wire it here up for hotkeys
    }
}
