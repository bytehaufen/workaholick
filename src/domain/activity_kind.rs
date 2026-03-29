use serde::{Deserialize, Serialize};

use super::WorkContext;

#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    strum::Display,
    strum::EnumString,
    strum::VariantNames,
)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    Work { context: WorkContext },
    Vacation,
    Holiday,
    Sick,
    SickChild,
}

impl Default for ActivityKind {
    fn default() -> Self {
        Self::Work { context: WorkContext::Office }
    }
}
