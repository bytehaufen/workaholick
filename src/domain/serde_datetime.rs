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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_invalid_date() {
        let invalid_date = "\"not a date\"";
        let result: Result<DateTime<Local>, _> = serde_json::from_str(invalid_date);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_valid_date() {
        let dt = Local::now();
        let s = dt.format(super::FORMAT).to_string();
        let json = format!("\"{}\"", s);
        let result: Result<DateTime<Local>, _> = serde_json::from_str(&json);
        assert!(result.is_ok());
    }
}
