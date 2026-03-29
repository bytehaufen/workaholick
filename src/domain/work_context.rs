#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Eq,
    PartialEq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    strum::Display,
    strum::EnumString,
    strum::VariantNames,
)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "lowercase")]
pub enum WorkContext {
    #[default]
    Home,
    Office,
    Travel,
}
