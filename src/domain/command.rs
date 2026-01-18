use super::ActivityKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, strum::Display, strum::EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    #[strum(to_string = "start")]
    Start { kind: ActivityKind },
    #[strum(to_string = "end")]
    End {
        title: String,
        description: Option<String>,
    },
    #[strum(to_string = "log")]
    Log {
        title: String,
        description: Option<String>,
    },
    #[strum(to_string = "start break")]
    BreakStart,
    #[strum(to_string = "end break")]
    BreakEnd { description: Option<String> },
}

impl Default for Command {
    fn default() -> Self {
        Self::Log {
            title: String::new(),
            description: None,
        }
    }
}
