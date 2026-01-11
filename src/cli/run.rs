use super::args::{Cli, CliCommand};
use crate::App;
use crate::editor::Editor;
use crate::{EntryStore, web};

use anyhow::Result;

pub fn run<S: EntryStore, E: Editor>(app: &mut App<S, E>, cli: Cli) -> Result<()> {
    if let Some(ref config_path) = cli.config {
        println!("Value for config: {}", config_path.display());
    }

    match cli.command {
        CliCommand::Config { command: _ } => anyhow::bail!("config handled in main"),
        CliCommand::Web => web::run(app),
        CliCommand::In { .. } => app.add(cli.command.try_into()?),
        CliCommand::Out { .. } => app.add(cli.command.try_into()?),
        CliCommand::Log { .. } => app.add(cli.command.try_into()?),
        CliCommand::BreakStart => app.add(cli.command.try_into()?),
        CliCommand::BreakEnd { .. } => app.add(cli.command.try_into()?),
        CliCommand::Delete { id } => app.delete(id),
        CliCommand::Edit { id } => app.edit(id),
        CliCommand::Status => {
            // FIXME: Handle errors
            match app.status()? {
                Some(summary) => println!("{:?}", summary),
                None => println!("No status for today"),
            };

            Ok(())
        }
        CliCommand::Day { date } => app.day(date),
        CliCommand::Report {} => todo!(),
    }
}
