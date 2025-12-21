use super::Command;
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    id: u64,
    version: u16,
    timestamp: DateTime<Local>,
    date: NaiveDate,
    payload: Command,
}

impl Entry {
    pub fn new(
        id: u64,
        version: u16,
        timestamp: DateTime<Local>,
        date: NaiveDate,
        payload: Command,
    ) -> Entry {
        Entry {
            id,
            version,
            timestamp,
            date,
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::DayType;

    use super::*;
    use chrono::{Local, TimeZone};

    #[test]
    fn entry_serializes_command_in() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let command = Command::In {
            day_type: DayType::Office,
        };
        let entry = Entry::new(1, 1, timestamp, timestamp.date_naive(), command);

        let json = serde_json::to_value(&entry).unwrap();

        let day_type = json["payload"]["In"]["day_type"].as_str().unwrap();
        assert_eq!(day_type, "office");

        let decoded: Entry = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, entry);
    }

    #[test]
    fn entry_serializes_command_out() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();

        let command = Command::Out {
            description: Some("test for Command::Out".to_string()),
        };

        let entry = Entry::new(1, 1, timestamp, timestamp.date_naive(), command);
        let json = serde_json::to_value(&entry).unwrap();

        let description = json["payload"]["Out"]["description"].as_str().unwrap();
        assert_eq!(description, "test for Command::Out");

        let decoded: Entry = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, entry);
    }

    #[test]
    fn entry_serializes_command_log() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let command = Command::Log {
            description: Some("test for Command::Log".to_string()),
        };
        let entry = Entry::new(1, 2, timestamp, timestamp.date_naive(), command);

        let json = serde_json::to_value(&entry).unwrap();

        let description = json["payload"]["Log"]["description"].as_str().unwrap();
        assert_eq!(description, "test for Command::Log");

        let decoded: Entry = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, entry);
    }
}
