use chrono::NaiveDate;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::domain::event::Place;

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
    /// Start a workday
    #[command(visible_alias = "i")]
    In {
        /// Where are you working?
        #[arg(value_name = "PLACE")]
        place: Place,
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
    /// Start a break
    #[command(visible_alias = "bs")]
    BreakStart,
    // End a break
    #[command(visible_alias = "be")]
    BreakEnd { description: Option<String> },
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

#[derive(Copy, Clone, Debug, ValueEnum)]
enum PlaceArg {
    Office,
    Home,
    Travel,
    Vacation,
}

impl From<PlaceArg> for Place {
    fn from(value: PlaceArg) -> Self {
        match value {
            PlaceArg::Office => Place::Office,
            PlaceArg::Home => Place::Home,
            PlaceArg::Travel => Place::Travel,
            PlaceArg::Vacation => Place::Vacation,
        }
    }
}
