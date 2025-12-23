use chrono::NaiveDate;
use clap::{Parser, Subcommand};
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
    // TODO: Implement more alternative frontends
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
        /// Optional description of what you finished
        description: Option<String>,
    },
    /// Finish a block of work
    #[command(visible_alias = "l")]
    Log {
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
        // ID of the entry to edit
        id: u64,

        /// Print the entry to stdout instead of editing
        #[arg(long)]
        print: bool,

        /// Apply a new description
        #[arg(long, value_name = "TEXT")]
        description: Option<String>,
    },
    /// Inspect a day
    #[command(visible_alias = "d")]
    Day {
        /// Date to Inspect
        date: NaiveDate,
    },
    /// Show summaries
    #[command(visible_alias = "r")]
    Report {
        // TODO: implement
    },
}
