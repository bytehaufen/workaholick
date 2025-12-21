use serde::{Deserialize, Serialize};
use thiserror::Error;

// TODO: Remove hardcoded variants
#[derive(Debug, Clone, Error)]
#[error("invalid day_type '{input}' (expected one of: office, home, travel, vacation, sick)")]
pub struct ParseDayTypeError {
    input: String,
}

#[derive(
    Copy, Clone, Debug, PartialEq, Serialize, Deserialize, strum::Display, strum::EnumString,
)]
#[strum(ascii_case_insensitive)]
#[serde(rename_all = "lowercase")]
pub enum DayType {
    Office,
    Home,
    Travel,
    Vacation,
    Sick,
}
