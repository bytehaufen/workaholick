use super::DayType;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    In {
        day_type: DayType,
    },
    Out {
        title: String,
        description: Option<String>,
    },
    Log {
        title: String,
        description: Option<String>,
    },
    BreakStart,
    BreakEnd {
        description: Option<String>,
    },
}
