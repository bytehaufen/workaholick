use crate::domain::model::Place;

#[derive(Debug)]
pub enum Command {
    In { place: Place },
    Out { description: Option<String> },
    Log { description: Option<String> },
    Pause { description: Option<String> },
    Day { date: chrono::NaiveDate },
}
