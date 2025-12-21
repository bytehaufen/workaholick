use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Copy, Clone, Debug, Serialize, Deserialize, strum::Display, strum::EnumString)]
#[strum(ascii_case_insensitive)]
#[serde(rename_all = "lowercase")]
pub enum DayType {
    Office,
    Home,
    Travel,
    Vacation,
    Sick,
}

// TODO: Customize if moved to config
#[derive(Debug, Clone, Error)]
#[error("invalid day_type '{input}' (expected one of: office, home, travel, vacation, sick)")]
pub struct ParseDayTypeError {
    input: String,
}

type Time = DateTime<Local>;

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandBase {
    id: u64,
    version: u16,
    timestamp: Time,
    date: NaiveDate,
}

impl CommandBase {
    pub fn new(id: u64, version: u16, timestamp: Time, date: NaiveDate) -> CommandBase {
        CommandBase {
            id,
            version,
            timestamp,
            date,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    In {
        #[serde(flatten)]
        base: CommandBase,
        day_type: DayType,
    },
    Out {
        #[serde(flatten)]
        base: CommandBase,
        description: Option<String>,
    },
    Log {
        #[serde(flatten)]
        base: CommandBase,
        description: Option<String>,
    },
    BreakStart {
        #[serde(flatten)]
        base: CommandBase,
    },
    BreakEnd {
        #[serde(flatten)]
        base: CommandBase,
        description: Option<String>,
    },
}
