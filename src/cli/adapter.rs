use anyhow::{Error, bail};

use super::args::CliCommand;
use crate::domain::Command;

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
