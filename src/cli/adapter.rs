use super::args::CliCommand;
use crate::domain::Command;

// TODO: TryFrom Really the right one here?
impl TryFrom<CliCommand> for Command {
    type Error = anyhow::Error;

    fn try_from(command: CliCommand) -> Result<Self, Self::Error> {
        Ok(match command {
            CliCommand::In { day_type } => Command::In { day_type },
            CliCommand::Out { description } => Command::Out { description },
            CliCommand::Log { description } => Command::Log { description },
            CliCommand::BreakStart => Command::BreakStart,
            CliCommand::BreakEnd { description } => Command::BreakEnd { description },

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
