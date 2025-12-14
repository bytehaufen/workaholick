use std::fmt::{self};
use std::str::FromStr;

use thiserror::Error;

// TODO: Implement
pub struct Task;
// TODO: Implement
pub struct Session;

#[derive(Copy, Clone, Debug)]
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
#[error("invalid place '{input}' (expected one of: office, home, travel, vacation")]
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
