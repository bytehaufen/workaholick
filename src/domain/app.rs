use crate::domain::Event;

#[derive(Debug, Clone, Default)]
pub struct App;

impl App {
    pub fn new() -> Self {
        App
    }

    pub fn execute(&self, cmd: Event) -> anyhow::Result<()> {
        match cmd {
            Event::In { place } => {
                println!("Core: starting work at {}", place);
            }
            Event::Out { description } => {
                println!("Core: stopping work: {:?}", description);
            }
            Event::Log { description } => {
                println!("Core: log: {:?}", description);
            }
            Event::Pause { description } => {
                println!("Core: pause: {:?}", description);
            }
            Event::Day { date } => {
                println!("Core: day {}", date);
            }
        }

        Ok(())
    }
}
