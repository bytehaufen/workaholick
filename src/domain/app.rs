use crate::domain::Event;

#[derive(Debug, Clone, Default)]
pub struct App;

impl App {
    pub fn new() -> Self {
        App
    }

    pub fn execute(&self, cmd: Event) -> anyhow::Result<()> {
        match cmd {
            Event::In { base, place } => {
                println!("Core: starting work at {} {:?}", place, base);
            }
            Event::Out { base, description } => {
                println!("Core: stopping work: {:?} {:?}", description, base);
            }
            Event::Log { base, description } => {
                println!("Core: log: {:?} {:?}", description, base);
            }
            Event::BreakStart { base } => {
                println!("Core: break {:?}", base);
            }
            Event::BreakEnd { base, description } => {
                println!("Core: break end: {:?} {:?}", description, base);
            }
        }

        Ok(())
    }
}
