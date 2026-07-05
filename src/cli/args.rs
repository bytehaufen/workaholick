use chrono::NaiveDate;
use clap::{
    ArgGroup, Args, ColorChoice, Parser, Subcommand, ValueEnum,
    builder::{PossibleValuesParser, TypedValueParser, styling::Style},
};
use clap_complete::Shell;
use std::path::PathBuf;
use strum::VariantNames;

use crate::domain::{
    AbsenceKind, AbsencePortion, AbsenceRecordInput, Period, WorkContext,
    activity_report::ActivityGroupBy, reporting::DateScope,
};

// FIXME: Update when domain implemented
const HEADER: Style = clap_cargo::style::HEADER;
fn examples_help() -> String {
    format!(
        "{HEADER}Examples:{HEADER:#}
  workaholick work start office
  workaholick work log \"Code review\" --description \"PR #184\"
  workaholick work end \"Wrap up\" --description \"Release prep\"
  workaholick absence vacation --date 2026-05-01
  workaholick absence sick --date 2026-05-01 --portion half-day --note \"doctor\"
  workaholick day
  workaholick timesheet week -1
  workaholick activity --from 2026-04-01 --to 2026-04-06 --group-by title
"
    )
}

#[derive(Parser, Debug)]
#[command(
    version,
    about,
    color = ColorChoice::Auto,
    styles= clap_cargo::style::CLAP_STYLING,
    after_help = examples_help()
)]
pub struct Cli {
    /// Path to the config file to use
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Command to execute
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Subcommand, Debug)]
pub enum CliCommand {
    /// Get and set config
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommand>,
    },

    /// Print shell completion script to stdout
    Completions {
        #[arg(value_enum)]
        shell: CompletionShell,
    },

    /// Track timed work activity
    #[command(visible_alias = "w")]
    Work {
        /// Work action to execute
        #[command(subcommand)]
        command: WorkSubcommand,
    },

    /// Record an absence for a date
    #[command(visible_alias = "a")]
    Absence {
        /// Kind of absence to record
        #[arg( value_parser = absence_kind_parser())]
        kind: AbsenceKind,

        /// Absence details
        #[command(flatten)]
        details: AbsenceDetailsArgs,
    },

    /// Delete one or more records
    Delete {
        /// ID of the record to delete
        id: u64,
    },

    /// Edit record
    Edit {
        /// ID of the record to edit
        id: u64,
    },

    /// Show all records for a day
    #[command(visible_alias = "d")]
    Day {
        /// Date to inspect (YYYY-MM-DD), latest day if left empty
        date: Option<NaiveDate>,
    },

    /// Summarize tracked time and breaks for a period or date range
    #[command(visible_alias = "sum")]
    Timesheet(TimesheetArgs),

    /// Show completed items for a period or date range
    #[command(visible_alias = "rep")]
    Activity(ActivityArgs),
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Show current config
    Show,

    /// Edit config file in $VISUAL/$EDITOR
    Edit,

    /// Create config file if missing
    Init,
}

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq)]
pub enum CompletionShell {
    #[value(name = "bash")]
    Bash,
    #[value(name = "elvish")]
    Elvish,
    #[value(name = "fish")]
    Fish,
    #[value(name = "powershell")]
    PowerShell,
    #[value(name = "zsh")]
    Zsh,
}

impl From<CompletionShell> for Shell {
    fn from(value: CompletionShell) -> Self {
        match value {
            CompletionShell::Bash => Self::Bash,
            CompletionShell::Elvish => Self::Elvish,
            CompletionShell::Fish => Self::Fish,
            CompletionShell::PowerShell => Self::PowerShell,
            CompletionShell::Zsh => Self::Zsh,
        }
    }
}

/// Actions within the `work` command namespace
#[derive(Debug, Clone, Subcommand)]
pub enum WorkSubcommand {
    /// Start tracking a time segment
    #[command(visible_alias = "s")]
    Start {
        /// Set the place of work
        #[arg( value_parser = work_context_parser())]
        context: WorkContext,
    },

    /// End the current active segment
    #[command(visible_alias = "e")]
    End {
        /// Title of what you finished
        title: String,
        /// Optional description of what you finished
        description: Option<String>,
    },

    /// Finish a block of work
    #[command(visible_alias = "l")]
    Log {
        /// Title of what you finished
        title: String,
        /// Optional description of what you finished
        description: Option<String>,
    },

    /// Start a break
    #[command(visible_alias = "bs")]
    BreakStart,

    /// End a break
    #[command(visible_alias = "be")]
    BreakEnd {
        /// Optional description for the break
        #[arg(long)]
        description: Option<String>,
    },
}

/// Shared arguments for absence creation
#[derive(Args, Debug, Clone)]
pub struct AbsenceDetailsArgs {
    /// Date of the absence (YYYY-MM-DD)
    #[arg(long)]
    pub date: NaiveDate,

    /// Full day by default, set to half-day when needed
    #[arg(long, value_enum, default_value = "full-day")]
    pub portion: AbsencePortion,

    /// Optional note for the absence
    #[arg(long)]
    pub note: Option<String>,
}

impl AbsenceDetailsArgs {
    pub(crate) fn into_input(self, kind: AbsenceKind) -> AbsenceRecordInput {
        AbsenceRecordInput::new(self.date, kind, self.portion, self.note)
    }
}

/// Scope selection shared by time sheet and activity commands
#[derive(Args, Debug, Clone)]
#[command(group(ArgGroup::new("scope").args(["period", "from"]).required(false)))]
pub struct ScopeArgs {
    /// Preset period to summarize
    #[arg(value_enum, conflicts_with_all = ["from", "to"])]
    #[arg( value_parser = period_parser())]
    pub period: Option<Period>,

    /// Relative period offset (zero or negative), e.g. `-1` for the previous period
    #[arg(
        value_name = "OFFSET",
        requires = "period",
        conflicts_with_all = ["from", "to"],
        allow_hyphen_values = true,
        value_parser = parse_period_offset
    )]
    pub offset: Option<i64>,

    /// Start date for an explicit range (YYYY-MM-DD)
    #[arg(long, requires = "to", conflicts_with = "period")]
    pub from: Option<NaiveDate>,

    /// End date for an explicit range (YYYY-MM-DD)
    #[arg(long, requires = "from", conflicts_with = "period")]
    pub to: Option<NaiveDate>,
}

impl ScopeArgs {
    pub fn scope(&self) -> DateScope {
        match (self.period, self.offset, self.from, self.to) {
            (Some(period), offset, None, None) => {
                DateScope::Period { period, offset: offset.unwrap_or(0) }
            }
            (None, None, Some(from), Some(to)) => DateScope::Range { from, to },
            (None, None, None, None) => DateScope::Period { period: Period::Week, offset: 0 },
            _ => unreachable!("clap validation should prevent invalid timesheet arguments"),
        }
    }
}

fn parse_period_offset(value: &str) -> Result<i64, String> {
    let offset = value.parse::<i64>().map_err(|_| format!("invalid integer: {value}"))?;

    if offset > 0 { Err("period offset must be zero or negative".to_string()) } else { Ok(offset) }
}

/// Arguments for the `timesheet` command
#[derive(Args, Debug, Clone)]
pub struct TimesheetArgs {
    #[command(flatten)]
    pub scope_args: ScopeArgs,
}

impl TimesheetArgs {
    pub fn scope(&self) -> DateScope {
        self.scope_args.scope()
    }
}

/// Arguments for the `activity` command
#[derive(Args, Debug, Clone)]
pub struct ActivityArgs {
    #[command(flatten)]
    pub scope_args: ScopeArgs,

    /// Group the activity rows
    #[arg(long, value_enum)]
    pub group_by: Option<ActivityGroupBy>,
}

impl ActivityArgs {
    pub fn scope(&self) -> DateScope {
        self.scope_args.scope()
    }
}

pub fn parse() -> Cli {
    Cli::parse()
}

fn work_context_parser() -> impl TypedValueParser<Value = WorkContext> {
    PossibleValuesParser::new(WorkContext::VARIANTS.iter().copied()).try_map(|s| s.parse())
}

fn absence_kind_parser() -> impl TypedValueParser<Value = AbsenceKind> {
    PossibleValuesParser::new(AbsenceKind::VARIANTS.iter().copied()).try_map(|s| s.parse())
}

fn period_parser() -> impl TypedValueParser<Value = Period> {
    PossibleValuesParser::new(Period::VARIANTS.iter().copied()).try_map(|s| s.parse())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn timesheet_scope_defaults_to_current_week() {
        let cli = Cli::try_parse_from(["workaholick", "timesheet"]).unwrap();

        let CliCommand::Timesheet(args) = cli.command else {
            panic!("expected timesheet command");
        };

        assert_eq!(args.scope(), DateScope::Period { period: Period::Week, offset: 0 });
    }

    #[test]
    fn timesheet_scope_parses_negative_period_offset() {
        let cli = Cli::try_parse_from(["workaholick", "timesheet", "week", "-1"]).unwrap();

        let CliCommand::Timesheet(args) = cli.command else {
            panic!("expected timesheet command");
        };

        assert_eq!(args.scope(), DateScope::Period { period: Period::Week, offset: -1 });
    }

    #[test]
    fn timesheet_scope_rejects_positive_period_offset() {
        let err = Cli::try_parse_from(["workaholick", "timesheet", "month", "1"]).unwrap_err();

        assert!(err.to_string().contains("period offset must be zero or negative"));
    }

    #[test]
    fn activity_scope_defaults_to_current_week() {
        let cli = Cli::try_parse_from(["workaholick", "activity"]).unwrap();

        let CliCommand::Activity(args) = cli.command else {
            panic!("expected activity command");
        };

        assert_eq!(args.scope(), DateScope::Period { period: Period::Week, offset: 0 });
        assert_eq!(args.group_by, None);
    }

    #[test]
    fn activity_scope_parses_grouping() {
        let cli =
            Cli::try_parse_from(["workaholick", "activity", "week", "-1", "--group-by", "title"])
                .unwrap();

        let CliCommand::Activity(args) = cli.command else {
            panic!("expected activity command");
        };

        assert_eq!(args.scope(), DateScope::Period { period: Period::Week, offset: -1 });
        assert_eq!(args.group_by, Some(ActivityGroupBy::Title));
    }

    #[test]
    fn activity_scope_parses_title_description_grouping() {
        let cli =
            Cli::try_parse_from(["workaholick", "activity", "--group-by", "title-description"])
                .unwrap();

        let CliCommand::Activity(args) = cli.command else {
            panic!("expected activity command");
        };

        assert_eq!(args.group_by, Some(ActivityGroupBy::TitleDescription));
    }

    #[test]
    fn completions_shell_parses() {
        let cli = Cli::try_parse_from(["workaholick", "completions", "zsh"]).unwrap();

        let CliCommand::Completions { shell } = cli.command else {
            panic!("expected completions command");
        };

        assert_eq!(shell, CompletionShell::Zsh);
    }
}
