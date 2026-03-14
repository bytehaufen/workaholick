use super::args::{Cli, CliCommand};
use super::edit_format::{parse_edited, render_for_edit};
use super::validation_error;
use crate::App;
use crate::domain::Command;
use crate::editor::{Editor, system::FileType};
use crate::{EntryReader, EntryWriter, web};

use anyhow::Result;

macro_rules! append_and_print {
    ($app:expr, $cmd:expr) => {{
        let id = $app.append($cmd)?;
        println!("✅ Entry with ID {id} added.");

        Ok(())
    }};
}

pub fn run<S: EntryReader + EntryWriter>(
    app: &mut App<S>,
    cli: Cli,
    editor: &dyn Editor,
) -> Result<()> {
    run_inner(app, cli, editor).map_err(validation_error::style_error)
}

fn run_inner<S: EntryReader + EntryWriter>(
    app: &mut App<S>,
    cli: Cli,
    editor: &dyn Editor,
) -> Result<()> {
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
            let entry = app.delete(id.into())?;
            println!("🗑️ Entry deleted:\n{entry:#?}");
            Ok(())
        }
        CliCommand::Edit { id } => {
            let id = id.into();
            let entry = app.get_entry_patch(id)?;
            let initial = render_for_edit(&entry)?;
            let edited = editor.edit(&initial, FileType::Ron)?;
            let patch = parse_edited(&edited)?;
            app.update_entry(id, patch)
        }

        CliCommand::Status => {
            // FIXME: implement
            // match app.status()? {
            //     Some(summary) => println!("{:?}", summary),
            //     None => println!("No status for today"),
            // };

            Ok(())
        }
        CliCommand::Day { date } => {
            let entries = app.day(date)?;
            println!("{entries:#?}");
            Ok(())
        }
        CliCommand::Report {} => todo!(),
    }
}
