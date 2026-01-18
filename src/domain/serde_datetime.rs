use chrono::{DateTime, Local};
use serde::{Deserialize, Deserializer, Serializer, de::Error};

const FORMAT: &str = "%Y-%m-%d %H:%M:%S %z";

pub fn serialize<S: Serializer>(
    date_time: &DateTime<Local>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let s = date_time.format(FORMAT).to_string();
    serializer.serialize_str(&s)
}

pub fn deserialize<'d, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
where
    D: Deserializer<'d>,
{
    let s = String::deserialize(deserializer)?;
    let date_time = DateTime::parse_from_str(&s, FORMAT).map_err(Error::custom)?;

    Ok(date_time.with_timezone(&Local))
}
