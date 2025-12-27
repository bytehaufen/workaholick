use super::Command;
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Copy, PartialOrd, Eq, Ord, Serialize, Deserialize)]
pub struct EntryId(u64);

impl EntryId {
    pub fn new(id: u64) -> EntryId {
        EntryId(id)
    }
}
//
// impl From<EntryId> for u64 {
//     fn from(id: EntryId) -> Self {
//         id.0
//     }
// }

impl From<u64> for EntryId {
    fn from(id: u64) -> Self {
        EntryId(id)
    }
}

impl EntryId {
    pub fn value(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct EntryVersion(pub(crate) u64);

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    id: EntryId,
    version: EntryVersion,
    timestamp: DateTime<Local>,
    date: NaiveDate,
    payload: Command,
}

impl Entry {
    pub(crate) fn new(
        id: EntryId,
        version: EntryVersion,
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

    pub(crate) fn from_new_entry(id: EntryId, version: EntryVersion, new: NewEntry) -> Entry {
        Entry {
            id,
            version,
            timestamp: new.timestamp,
            date: new.date,
            payload: new.payload,
        }
    }

    pub fn id(&self) -> &EntryId {
        &self.id
    }

    pub fn payload(&self) -> &Command {
        &self.payload
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct NewEntry {
    timestamp: DateTime<Local>,
    date: NaiveDate,
    payload: Command,
}

impl NewEntry {
    pub fn new(timestamp: DateTime<Local>, payload: Command) -> Self {
        NewEntry {
            timestamp,
            date: timestamp.date_naive(),
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
        let entry = Entry::new(
            EntryId(1),
            EntryVersion(1),
            timestamp,
            timestamp.date_naive(),
            command,
        );

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

        let entry = Entry::new(
            EntryId(1),
            EntryVersion(1),
            timestamp,
            timestamp.date_naive(),
            command,
        );
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
        let entry = Entry::new(
            EntryId(1),
            EntryVersion(1),
            timestamp,
            timestamp.date_naive(),
            command,
        );

        let json = serde_json::to_value(&entry).unwrap();

        let description = json["payload"]["Log"]["description"].as_str().unwrap();
        assert_eq!(description, "test for Command::Log");

        let decoded: Entry = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, entry);
    }

    #[test]
    fn entry_serializes_command_break_start() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let command = Command::BreakStart;
        let entry = Entry::new(
            EntryId(1),
            EntryVersion(1),
            timestamp,
            timestamp.date_naive(),
            command,
        );

        let json = serde_json::to_value(&entry).unwrap();

        let break_start = json["payload"].as_str().unwrap();
        assert_eq!(break_start, "BreakStart");

        let decoded: Entry = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, entry);
    }

    #[test]
    fn entry_serializes_command_break_end() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let command = Command::BreakEnd {
            description: Some("test for Command::BreakEnd".to_string()),
        };
        let entry: Entry = Entry::new(
            EntryId(42),
            EntryVersion(4),
            timestamp,
            timestamp.date_naive(),
            command,
        );

        let json = serde_json::to_value(&entry).unwrap();
        let break_end = json["payload"]["BreakEnd"]["description"].as_str().unwrap();
        assert_eq!(break_end, "test for Command::BreakEnd");

        let decoded: Entry = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, entry);
    }
}
