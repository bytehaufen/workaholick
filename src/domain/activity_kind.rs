use serde::{Deserialize, Serialize};

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    Default,
    strum::Display,
    strum::EnumString,
    strum::VariantNames,
)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "lowercase")]
pub enum ActivityKind {
    #[default]
    Work,
    Vacation,
    Holiday,
    Sick,
    ChildSick,
}
