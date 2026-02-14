use super::args::{Cli, CliCommand};
use crate::App;
use crate::domain::Command;
use crate::editor::Editor;
use crate::{EntryStore, web};

use anyhow::Result;

macro_rules! append_and_print {
    ($app:expr, $cmd:expr) => {{
        let id = $app.append($cmd)?;
        println!("✅ Entry with ID {id} added.");

        Ok(())
    }};
}

pub fn run<S: EntryStore, E: Editor>(app: &mut App<S, E>, cli: Cli) -> Result<()> {
    if let Some(ref config_path) = cli.config {
        println!("Value for config: {}", config_path.display());
    }

    match cli.command {
        CliCommand::Web => web::run(app),

        CliCommand::Config { .. } => anyhow::bail!("config handled in main"),

        CliCommand::Start { kind } => append_and_print!(app, Command::Start { kind: kind.into() }),
        CliCommand::End { title, description } => {
            append_and_print!(app, Command::End { title, description })
        }
        CliCommand::Log { title, description } => {
            append_and_print!(app, Command::Log { title, description })
        }
        CliCommand::BreakStart => append_and_print!(app, Command::BreakStart),
        CliCommand::BreakEnd { description } => {
            append_and_print!(app, Command::BreakEnd { description })
        }

        CliCommand::Delete { id } => {
            let entry = app.delete(id)?;
            println!("🗑️ Entry deleted:\n{entry:#?}");
            Ok(())
        }
        CliCommand::Edit { id } => app.edit(id),

        CliCommand::Status => {
            // FIXME: implement
            // match app.status()? {
            //     Some(summary) => println!("{:?}", summary),
            //     None => println!("No status for today"),
            // };

            Ok(())
        }
        CliCommand::Day { date } => app.day(date),
        CliCommand::Report {} => todo!(),
    }
}
