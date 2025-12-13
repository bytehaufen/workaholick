use chrono::NaiveDate;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start a workday
    #[command(visible_alias = "i")]
    In {
        /// Where are you working?
        #[arg(value_enum)]
        place: Place,

        /// Optional initial description for what you're starting with
        description: Option<String>,
    },
    /// End the current workday
    #[command(visible_alias = "o")]
    Out {
        /// What did you finish with?
        description: Option<String>,
    },
    /// Finish a block of work
    #[command(visible_alias = "l")]
    Log {
        /// What have you finished?
        description: Option<String>,
    },
    /// Start or log a break
    #[command(visible_alias = "p")]
    Pause,
    /// Inspect a day
    #[command(visible_alias = "d")]
    Day { date: NaiveDate },
    /// Show summaries
    #[command(visible_alias = "r")]
    Report {
        // TODO: implement
    },
    /// Edit entry
    #[command(visible_alias = "e")]
    Edit {
        // TODO: implement
    },
    /// Delete entry / entries
    #[command(visible_alias = "rm")]
    Delete {
        // TODO: implement
    },
    /// Start in web mode
    #[command()]
    Web, // TODO: Implement more alternative frontends
}

// TODO: Move into settings as default
#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum Place {
    Office,
    Home,
    Travel,
    Vacation,
}
