use super::DayType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    In {
        #[serde(flatten)]
        day_type: DayType,
    },
    Out {
        #[serde(flatten)]
        description: Option<String>,
    },
    Log {
        #[serde(flatten)]
        description: Option<String>,
    },
    BreakStart,
    BreakEnd {
        #[serde(flatten)]
        description: Option<String>,
    },
}
