use crate::domain::WorkContext;

use super::ActivityKind;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumIter, strum::Display)]
pub enum Command {
    #[strum(to_string = "start")]
    Start {
        kind: ActivityKind,
        context: Option<WorkContext>,
    },
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
