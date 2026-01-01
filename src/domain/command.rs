use super::DayType;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumIter)]
#[serde(tag = "type")]
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

impl Default for Command {
    fn default() -> Self {
        Self::Log {
            title: "title".into(),
            description: None,
        }
    }
}
