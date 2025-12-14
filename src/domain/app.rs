use crate::domain;

#[derive(Debug, Clone, Default)]
pub struct App;

impl App {
    pub fn new() -> Self {
        App
    }

    pub fn execute(&self, cmd: domain::Command) -> anyhow::Result<()> {
        match cmd {
            domain::Command::In { place } => {
                println!("Core: starting work at {}", place);
            }
            domain::Command::Out { description } => {
                println!("Core: stopping work: {:?}", description);
            }
            domain::Command::Log { description } => {
                println!("Core: log: {:?}", description);
            }
            domain::Command::Pause { description } => {
                println!("Core: pause: {:?}", description);
            }
            domain::Command::Day { date } => {
                println!("Core: day {}", date);
            }
        }

        Ok(())
    }
}
