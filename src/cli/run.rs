use super::args::{Cli, CliCommand};
use crate::App;
use crate::domain::Command;
use crate::editor::Editor;
use crate::{EntryStore, web};

use anyhow::Result;

pub fn run<S: EntryStore, E: Editor>(app: &mut App<S, E>, cli: Cli) -> Result<()> {
    if let Some(ref config_path) = cli.config {
        println!("Value for config: {}", config_path.display());
    }

    match cli.command {
        CliCommand::Web => web::run(app),

        CliCommand::Config { .. } => anyhow::bail!("config handled in main"),

        CliCommand::Start { kind } => app.add(Command::Start { kind: kind.into() }),
        CliCommand::End { title, description } => app.add(Command::End { title, description }),
        CliCommand::Log { title, description } => app.add(Command::Log { title, description }),
        CliCommand::BreakStart => app.add(Command::BreakStart),
        CliCommand::BreakEnd { description } => app.add(Command::BreakEnd { description }),

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
