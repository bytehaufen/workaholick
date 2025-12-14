use super::args::CliCommand;
use crate::domain::Event;

impl TryFrom<CliCommand> for Event {
    type Error = anyhow::Error;

    fn try_from(command: CliCommand) -> Result<Self, Self::Error> {
        Ok(match command {
            CliCommand::In { place } => Event::In { place },
            CliCommand::Out { description } => Event::Out { description },
            CliCommand::Log { description } => Event::Log { description },
            CliCommand::Pause { description } => Event::Pause { description },
            CliCommand::Day { date } => Event::Day { date },
            CliCommand::Report {} => todo!(),
            CliCommand::Edit {} => todo!(),
            CliCommand::Delete {} => todo!(),
            CliCommand::Web => {
                anyhow::bail!("Web is handled by the frontend, not the core adapter.")
            }
        })
    }
}
