use super::args::{Cli, CliCommand};
use super::edit_format::{parse_edited, render_for_edit};
use super::format;
use crate::App;
use crate::config::OutputFormatter;
use crate::domain::Command;
use crate::editor::{Editor, ports::FileType};
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
    formatter: OutputFormatter,
) -> Result<()> {
    run_inner(app, cli, editor, formatter)
}

fn run_inner<S: EntryReader + EntryWriter>(
    app: &mut App<S>,
    cli: Cli,
    editor: &dyn Editor,
    formatter: OutputFormatter,
) -> Result<()> {
    match cli.command {
        CliCommand::Web => web::run(app),

        CliCommand::Config { .. } => anyhow::bail!("config handled in main"),
        CliCommand::Completions { .. } => anyhow::bail!("completions handled in main"),

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

        CliCommand::Day { date } => {
            let Some(date) = date.or_else(|| app.last_date()) else {
                anyhow::bail!("There are no entries.");
            };

            let entries = app.day(date)?;
            if entries.is_empty() {
                println!("No entries for {date}.");
                return Ok(());
            }

            format::print_day(formatter, date, &entries)
        }

        CliCommand::Summary(summary_args) => {
            let report = app.summary(summary_args.scope())?;
            format::print_summary(formatter, &report)
        }
        CliCommand::Report(report_args) => {
            let scope = report_args.scope();
            let report = app.report(scope.clone())?;
            let summary = if report_args.summary { Some(app.report_summary(scope)?) } else { None };

            format::print_report(
                formatter,
                &report,
                report_args.group_by.map(Into::into),
                summary.as_ref(),
            )
        }
    }
}
