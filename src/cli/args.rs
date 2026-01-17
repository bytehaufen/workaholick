use chrono::NaiveDate;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::domain::{ActivityKind, Command, Period, WorkContext};

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Subcommand, Debug, strum::Display)]
#[strum(serialize_all = "kebab-case")]
pub enum CliCommand {
    /// Start in web mode
    #[command()]
    Web,

    /// Get and set config
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommand>,
    },

    // --- Write commands  ---
    /// Start tracking a time segment
    #[command(visible_alias = "start")]
    Start {
        // Start tracking a time segment
        #[arg(value_enum, value_name = "KIND")]
        kind: ActivityKindArg,

        /// Set the place of work (only valid when KIND=work)
        #[arg(long, value_enum, value_name = "CONTEXT")]
        context: Option<WorkContextArg>,
    },

    /// End the current active segment
    #[command(visible_alias = "end")]
    End {
        // Title of what you finished
        title: String,
        /// Optional description of what you finished
        description: Option<String>,
    },

    /// Finish a block of work
    #[command(visible_alias = "l")]
    Log {
        // Title of what you finished
        title: String,
        /// Optional description of what you finished
        description: Option<String>,
    },

    /// Delete one or more entries
    #[command(visible_alias = "rm")]
    Delete {
        /// ID of the entry to delete
        id: u64,
    },

    /// Edit entry
    #[command(visible_alias = "e")]
    Edit {
        /// ID of the entry to edit
        id: u64,
    },

    // --- Query commands ---
    /// Status summary for today
    #[command(visible_alias = "s")]
    Status,

    /// Inspect a day (arbitrary date)
    #[command(visible_alias = "d")]
    Day {
        /// Date to inspect (YYYY-MM-DD)
        date: NaiveDate,
    },

    // TODO: Check necessity
    /// Show summaries
    #[command(visible_alias = "r")]
    Report {
        // TODO: implement
    },
}

impl CliCommand {
    fn validate(&self) -> anyhow::Result<()> {
        match self {
            CliCommand::Start { kind, context } => {
                if *kind != ActivityKindArg::Work && context.is_some() {
                    anyhow::bail!("--context is only valid when KIND=work")
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl TryFrom<CliCommand> for Command {
    type Error = anyhow::Error;

    fn try_from(cli_command: CliCommand) -> anyhow::Result<Self> {
        let command = match cli_command {
            CliCommand::Start { kind, context } => Command::Start {
                kind: kind.into(),
                context: context.map(Into::into),
            },
            CliCommand::End { title, description } => Command::End { title, description },
            CliCommand::Log { title, description } => Command::Log { title, description },

            CliCommand::Web
            | CliCommand::Config { .. }
            | CliCommand::Delete { .. }
            | CliCommand::Edit { .. }
            | CliCommand::Status
            | CliCommand::Day { .. }
            | CliCommand::Report {} => anyhow::bail!("'{cli_command}' is not a domain command"),
        };

        #[rustfmt::skip]
        let validate_all_variants_covered = |command: &Command| match *command {
            Command::Start { .. } | Command::End { .. } | Command::Log { .. } | Command::BreakStart | Command::BreakEnd { .. } => {}
        };

        validate_all_variants_covered(&command);

        Ok(command)
    }
}

#[derive(Copy, Clone, Debug, ValueEnum, PartialEq)]
pub enum ActivityKindArg {
    Work,
    Vacation,
    Holiday,
    Sick,
    ChildSick,
}

impl From<ActivityKindArg> for ActivityKind {
    fn from(time_kind_arg: ActivityKindArg) -> Self {
        match time_kind_arg {
            ActivityKindArg::Work => Self::Work,
            ActivityKindArg::Vacation => Self::Vacation,
            ActivityKindArg::Sick => Self::Sick,
            ActivityKindArg::ChildSick => Self::ChildSick,
            ActivityKindArg::Holiday => Self::Holiday,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum WorkContextArg {
    Home,
    Office,
    Travel,
}

impl From<WorkContextArg> for WorkContext {
    fn from(work_context_arg: WorkContextArg) -> Self {
        match work_context_arg {
            WorkContextArg::Home => Self::Home,
            WorkContextArg::Office => Self::Office,
            WorkContextArg::Travel => Self::Travel,
        }
    }
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

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum PeriodArg {
    /// Today (default)
    #[value(alias = "t")]
    Today,

    /// Previous logged day
    #[value(alias = "y")]
    Prev,

    /// This week
    #[value(alias = "w")]
    Week,

    /// This month
    #[value(alias = "m")]
    Month,
}

impl From<PeriodArg> for Period {
    fn from(p: PeriodArg) -> Self {
        match p {
            PeriodArg::Today => Period::Today,
            PeriodArg::Prev => Period::Prev,
            PeriodArg::Week => Period::Week,
            PeriodArg::Month => Period::Month,
        }
    }
}

pub fn parse() -> Cli {
    Cli::parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_command_to_command_conversion_errors() {
        // Test Web command
        let cli_web = CliCommand::Web;
        let result = Command::try_from(cli_web);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "'web' is not a domain command"
        );

        // Test Config command
        let cli_config = CliCommand::Config { command: None };
        let result = Command::try_from(cli_config);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "'config' is not a domain command"
        );
    }
}
