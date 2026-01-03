use chrono::NaiveDate;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::domain::DayType;

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Subcommand, Debug)]
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
    /// Start a workday
    #[command(visible_alias = "i")]
    In {
        /// Work location/type for this day
        #[arg(value_name = "DAY_TYPE")]
        day_type: DayType,
    },

    /// End the current workday
    #[command(visible_alias = "o")]
    Out {
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

pub fn parse() -> Cli {
    Cli::parse()
}
