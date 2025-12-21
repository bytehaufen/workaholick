use super::DayType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    In {
        #[serde(flatten)]
        day_type: DayType,
    },
    Out {
        description: Option<String>,
    },
    Log {
        description: Option<String>,
    },
    BreakStart,
    BreakEnd {
        description: Option<String>,
    },
}
