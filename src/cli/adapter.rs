use super::args;
use crate::domain;

impl TryFrom<args::Command> for domain::Command {
    type Error = anyhow::Error;

    fn try_from(command: args::Command) -> Result<Self, Self::Error> {
        Ok(match command {
            args::Command::In { place } => domain::Command::In { place },
            args::Command::Out { description } => domain::Command::Out { description },
            args::Command::Log { description } => domain::Command::Log { description },
            args::Command::Pause { description } => domain::Command::Pause { description },
            args::Command::Day { date } => domain::Command::Day { date },
            args::Command::Report {} => todo!(),
            args::Command::Edit {} => todo!(),
            args::Command::Delete {} => todo!(),
            args::Command::Web => {
                anyhow::bail!("Web is handled by the frontend, not the core adapter.")
            }
        })
    }
}
