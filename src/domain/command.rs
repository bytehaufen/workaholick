use super::DayType;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    In { day_type: DayType },
    Out { description: Option<String> },
    Log { description: Option<String> },
    BreakStart,
    BreakEnd { description: Option<String> },
}
