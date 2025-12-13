use crate::core::command::Command;

#[derive(Debug, Clone, Default)]
pub struct App;

impl App {
    pub fn new() -> Self {
        App
    }

    pub fn execute(&self, cmd: Command) -> anyhow::Result<()> {
        match cmd {
            Command::In { description } => {
                println!("Core: starting work: {}", description);
            }
            Command::Out => {
                println!("Core: stopping work");
            }
            Command::Status => {
                println!("Core: showing current status");
            }
        }

        Ok(())
    }
}
