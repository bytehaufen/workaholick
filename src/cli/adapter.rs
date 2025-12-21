use chrono::Local;

use super::args::CliCommand;
use crate::domain::{Command, command::CommandBase};

// TODO: TryFrom Really the right one here?
impl TryFrom<CliCommand> for Command {
    type Error = anyhow::Error;

    fn try_from(command: CliCommand) -> Result<Self, Self::Error> {
        // TODO: Extract and calculate correctly
        let timestamp = Local::now();
        let date = timestamp.date_naive();
        let base = CommandBase::new(42, 815, timestamp, date);

        Ok(match command {
            CliCommand::In { day_type } => Command::In { base, day_type },
            CliCommand::Out { description } => Command::Out { base, description },
            CliCommand::Log { description } => Command::Log { base, description },
            CliCommand::BreakStart => Command::BreakStart { base },
            CliCommand::BreakEnd { description } => Command::BreakEnd { base, description },

            // TODO: Implement informational stuff
            CliCommand::Day { date: _ } => todo!(),
            CliCommand::Report {} => todo!(),
            CliCommand::Edit {} => todo!(),
            CliCommand::Delete {} => todo!(),
            CliCommand::Web => {
                anyhow::bail!("Web is handled by the frontend, not the core adapter.")
            }
        })
    }
}
