use chrono::NaiveDate;
use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::domain::{ActivityKind, Period, SummaryScope, WorkContext};

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
    /// Inspect a day (arbitrary date)
    #[command(visible_alias = "d")]
    Day {
        /// Date to inspect (YYYY-MM-DD), latest day if left empty
        date: Option<NaiveDate>,
    },

    /// Show summaries
    #[command(visible_alias = "sum")]
    Summary(SummaryArgs),
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
            ActivityKindCommand::Work { context } => Self::Work { context: context.into() },
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

#[derive(Args, Debug, Clone)]
#[command(group(ArgGroup::new("summary_scope").args(["period", "from"]).required(false)))]
pub struct SummaryArgs {
    /// Preset period to summarize
    #[arg(value_enum, conflicts_with_all = ["from", "to"])]
    pub period: Option<PeriodArg>,

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

impl SummaryArgs {
    pub fn scope(&self) -> SummaryScope {
        match (self.period, self.offset, self.from, self.to) {
            (Some(period), offset, None, None) => {
                SummaryScope::Period { period: Period::from(period), offset: offset.unwrap_or(0) }
            }
            (None, None, Some(from), Some(to)) => SummaryScope::Range { from, to },
            (None, None, None, None) => SummaryScope::Period { period: Period::Day, offset: 0 },
            _ => unreachable!("clap validation should prevent invalid summary arguments"),
        }
    }
}

fn parse_period_offset(value: &str) -> Result<i64, String> {
    let offset = value.parse::<i64>().map_err(|_| format!("invalid integer: {value}"))?;

    if offset > 0 { Err("period offset must be zero or negative".to_string()) } else { Ok(offset) }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum PeriodArg {
    /// Today (default)
    #[value(alias = "d")]
    Day,

    /// This week
    #[value(alias = "w")]
    Week,

    /// This month
    #[value(alias = "m")]
    Month,

    /// This year
    #[value(alias = "y")]
    Year,
}

impl From<PeriodArg> for Period {
    fn from(p: PeriodArg) -> Self {
        match p {
            PeriodArg::Day => Period::Day,
            PeriodArg::Week => Period::Week,
            PeriodArg::Month => Period::Month,
            PeriodArg::Year => Period::Year,
        }
    }
}

pub fn parse() -> Cli {
    Cli::parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn summary_scope_defaults_to_current_day() {
        let cli = Cli::try_parse_from(["workaholick", "summary"]).unwrap();

        let CliCommand::Summary(args) = cli.command else {
            panic!("expected summary command");
        };

        assert_eq!(args.scope(), SummaryScope::Period { period: Period::Day, offset: 0 });
    }

    #[test]
    fn summary_scope_parses_negative_period_offset() {
        let cli = Cli::try_parse_from(["workaholick", "summary", "week", "-1"]).unwrap();

        let CliCommand::Summary(args) = cli.command else {
            panic!("expected summary command");
        };

        assert_eq!(args.scope(), SummaryScope::Period { period: Period::Week, offset: -1 });
    }

    #[test]
    fn summary_scope_rejects_positive_period_offset() {
        let err = Cli::try_parse_from(["workaholick", "summary", "month", "1"]).unwrap_err();

        assert!(err.to_string().contains("summary period offset must be zero or negative"));
    }
}
