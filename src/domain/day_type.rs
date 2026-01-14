use serde::{Deserialize, Serialize};

#[derive(
    Default,
    Copy,
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    strum::Display,
    strum::EnumString,
    strum::VariantNames,
    clap::ValueEnum,
)]
#[strum(ascii_case_insensitive)]
#[serde(rename_all = "lowercase")]
pub enum DayType {
    Office,
    #[default]
    Home,
    Travel,
    Vacation,
    Sick,
    Holiday,
}
