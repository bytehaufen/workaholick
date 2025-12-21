use core::fmt;
use std::str::FromStr;

use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Copy, Clone, Debug, Serialize, Deserialize, strum::Display)]
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

impl FromStr for DayType {
    type Err = ParseDayTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "office" => Ok(DayType::Office),
            "home" => Ok(DayType::Home),
            "travel" => Ok(DayType::Travel),
            "vacation" => Ok(DayType::Vacation),
            "sick" => Ok(DayType::Sick),
            _ => Err(ParseDayTypeError {
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
        day_type: DayType,
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
