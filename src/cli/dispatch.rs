use super::args::{Cli, CliCommand};
use super::edit_format::{parse_edited, render_for_edit};
use super::format;
use crate::App;
use crate::cli::args::{AbsenceDetailsArgs, WorkSubcommand};
use crate::config::OutputFormatter;
use crate::domain::RecordId;
use crate::editor::{Editor, ports::FileType};
use crate::store::RecordRepository;

use anyhow::Result;

pub(crate) fn run<S: RecordRepository>(
    app: &mut App<S>,
    cli: Cli,
    editor: &dyn Editor,
    formatter: OutputFormatter,
) -> Result<()> {
    run_inner(app, cli, editor, formatter)
}

fn run_inner<S: RecordRepository>(
    app: &mut App<S>,
    cli: Cli,
    editor: &dyn Editor,
    formatter: OutputFormatter,
) -> Result<()> {
    match cli.command {
        CliCommand::Config { .. } => anyhow::bail!("config handled in main"),
        CliCommand::Completions { .. } => anyhow::bail!("completions handled in main"),

        CliCommand::Work { command } => {
            let id = match command {
                WorkSubcommand::Start { context } => app.start_work(context)?,
                WorkSubcommand::End { title, description } => app.end_work(title, description)?,
                WorkSubcommand::Log { title, description } => app.log_work(title, description)?,
                WorkSubcommand::BreakStart => app.start_break()?,
                WorkSubcommand::BreakEnd { description } => app.end_break(description)?,
            };

            println!("✅ Record with ID {id} added.");
            Ok(())
        }
        CliCommand::Absence { details, dates } => {
            let range = dates.into_range()?;
            let AbsenceDetailsArgs { kind, portion, note } = details;

            let ids = app.add_absence_range(kind, portion, note, range)?;
            let ids_str = ids.iter().map(RecordId::to_string).collect::<Vec<_>>().join(", ");
            println!("✅ Record with ID(s) {ids_str} added.");
            Ok(())
        }
        CliCommand::Delete { id } => {
            let record = app.delete_record(id.into())?;
            println!("🗑️ Record deleted:\n{record:#?}");
            Ok(())
        }
        CliCommand::Edit { id } => {
            let id = id.into();
            let record = app
                .get_record_input(id)
                .ok_or_else(|| anyhow::anyhow!("No record with ID {id}"))?;
            let initial = render_for_edit(&record)?;
            let edited = editor.edit(&initial, FileType::Ron)?;
            let patch = parse_edited(&edited)?;
            let _ = app.update_record(id, patch)?;
            println!("✅ Record updated:\n{record:#?}");

            Ok(())
        }
        CliCommand::Day { date } => {
            let Some(date) = date.or_else(|| app.last_record_date()) else {
                anyhow::bail!("There are no records.");
            };

            let day_overview = app.day(date);
            if day_overview.is_empty() {
                println!("No records for {date}.");
                return Ok(());
            }

            format::print_day(formatter, date, &day_overview)
        }
        CliCommand::Timesheet(timesheet_args) => {
            let timesheet = app.timesheet(timesheet_args.scope())?;
            format::print_timesheet(formatter, &timesheet)
        }
        CliCommand::Activity(activity_args) => {
            let scope = activity_args.scope();
            let activity_report = app.activity(scope.clone())?;

            format::print_activity(formatter, &activity_report, activity_args.group_by_title)
        }
    }
}
