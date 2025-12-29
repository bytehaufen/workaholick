use anyhow::{Error, bail};

use super::args::{CliCommand, PeriodArg};
use crate::domain::{Command, Period};

// TODO: TryFrom Really the right one here?
impl TryFrom<CliCommand> for Command {
    type Error = Error;

    fn try_from(command: CliCommand) -> Result<Self, Self::Error> {
        Ok(match command {
            CliCommand::In { day_type } => Command::In { day_type },
            CliCommand::Out { title, description } => Command::Out { title, description },
            CliCommand::Log { title, description } => Command::Log { title, description },
            CliCommand::BreakStart => Command::BreakStart,
            CliCommand::BreakEnd { description } => Command::BreakEnd { description },
            _ => bail!("Other commands are handled in app, not the core adapter"),
        })
    }
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
