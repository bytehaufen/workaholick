use core::fmt;
use std::str::FromStr;

use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum Place {
    Office,
    Home,
    Travel,
    Vacation,
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Place::Office => "office",
            Place::Home => "home",
            Place::Travel => "travel",
            Place::Vacation => "vacation",
        })
    }
}

// TODO: Customize if moved to config
#[derive(Debug, Clone, Error)]
#[error("invalid place '{input}' (expected one of: office, home, travel, vacation)")]
pub struct ParsePlaceError {
    input: String,
}

impl FromStr for Place {
    type Err = ParsePlaceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "office" => Ok(Place::Office),
            "home" => Ok(Place::Home),
            "travel" => Ok(Place::Travel),
            "vacation" => Ok(Place::Vacation),
            _ => Err(ParsePlaceError {
                input: s.to_string(),
            }),
        }
    }
}

type Time = DateTime<Local>;

#[derive(Debug, Serialize, Deserialize)]
pub struct EventBase {
    id: u64,
    version: u16,
    timestamp: Time,
    date: NaiveDate,
}

impl EventBase {
    pub fn new(id: u64, version: u16, timestamp: Time, date: NaiveDate) -> EventBase {
        EventBase {
            id,
            version,
            timestamp,
            date,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Event {
    In {
        #[serde(flatten)]
        base: EventBase,
        place: Place,
    },
    Out {
        #[serde(flatten)]
        base: EventBase,
        description: Option<String>,
    },
    Log {
        #[serde(flatten)]
        base: EventBase,
        description: Option<String>,
    },
    BreakStart {
        #[serde(flatten)]
        base: EventBase,
    },
    BreakEnd {
        #[serde(flatten)]
        base: EventBase,
        description: Option<String>,
    },
}
