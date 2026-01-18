use chrono::NaiveDate;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::domain::{ActivityKind, Period, WorkContext};

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
    #[command(visible_alias = "s")]
    Start {
        #[command(subcommand)]
        kind: ActivityKindCommand,
    },

    /// End the current active segment
    #[command(visible_alias = "e")]
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

    /// Start a break                          
    #[command(visible_alias = "bs")]
    BreakStart,

    /// End a break                            
    #[command(visible_alias = "be")]
    BreakEnd {
        /// Optional description for the break
        description: Option<String>,
    },

    /// Delete one or more entries
    Delete {
        /// ID of the entry to delete
        id: u64,
    },

    /// Edit entry
    Edit {
        /// ID of the entry to edit
        id: u64,
    },

    // --- Query commands ---
    /// Status summary for today
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

#[derive(Copy, Clone, Debug, PartialEq, Subcommand)]
pub enum ActivityKindCommand {
    Work {
        /// Set the place of work
        context: WorkContextCommand,
    },
    Vacation,
    Holiday,
    Sick,
    SickChild,
}

impl From<ActivityKindCommand> for ActivityKind {
    fn from(time_kind_arg: ActivityKindCommand) -> Self {
        match time_kind_arg {
            ActivityKindCommand::Work { context } => Self::Work {
                context: context.into(),
            },
            ActivityKindCommand::Vacation => Self::Vacation,
            ActivityKindCommand::Sick => Self::Sick,
            ActivityKindCommand::SickChild => Self::SickChild,
            ActivityKindCommand::Holiday => Self::Holiday,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum WorkContextCommand {
    Home,
    Office,
    Travel,
}

impl From<WorkContextCommand> for WorkContext {
    fn from(work_context_arg: WorkContextCommand) -> Self {
        match work_context_arg {
            WorkContextCommand::Home => Self::Home,
            WorkContextCommand::Office => Self::Office,
            WorkContextCommand::Travel => Self::Travel,
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
    PrevDay,

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
            PeriodArg::PrevDay => Period::PrevDay,
            PeriodArg::Week => Period::Week,
            PeriodArg::Month => Period::Month,
        }
    }
}

pub fn parse() -> Cli {
    Cli::parse()
}
