use super::{Command, date_time_codec};
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Copy, PartialOrd, Eq, Ord, Serialize, Deserialize)]
pub struct EntryId(u64);

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
pub struct EntryVersion(u8);

impl From<u8> for EntryVersion {
    fn from(id: u8) -> Self {
        EntryVersion(id)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    id: EntryId,
    version: EntryVersion,
    #[serde(with = "date_time_codec")]
    timestamp: DateTime<Local>,
    date: NaiveDate,
    command: Command,
}

impl Entry {
    pub(crate) fn new(
        id: EntryId,
        version: EntryVersion,
        timestamp: DateTime<Local>,
        date: NaiveDate,
        command: Command,
    ) -> Entry {
        Entry {
            id,
            version,
            timestamp,
            date,
            command,
        }
    }

    pub(crate) fn from_new_entry(id: EntryId, version: EntryVersion, new: NewEntry) -> Entry {
        Self::new(id, version, new.timestamp, new.date, new.command)
    }

    pub fn id(&self) -> &EntryId {
        &self.id
    }

    pub fn command(&self) -> &Command {
        &self.command
    }

    pub fn version(&self) -> &EntryVersion {
        &self.version
    }

    pub fn timestamp(&self) -> &DateTime<Local> {
        &self.timestamp
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct NewEntry {
    #[serde(with = "date_time_codec")]
    timestamp: DateTime<Local>,
    date: NaiveDate,
    command: Command,
}

impl From<&Entry> for NewEntry {
    fn from(entry: &Entry) -> Self {
        let timestamp = entry.timestamp;

        Self {
            timestamp,
            date: timestamp.date_naive(),
            command: entry.command.clone(),
        }
    }
}

impl NewEntry {
    pub fn new(timestamp: DateTime<Local>, command: Command) -> Self {
        NewEntry {
            timestamp,
            date: timestamp.date_naive(),
            command,
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
        assert_eq!(json["command"]["type"].as_str().unwrap(), "In");
        assert_eq!(json["command"]["day_type"].as_str().unwrap(), "office");

        let decoded: Entry = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, entry);
    }

    #[test]
    fn entry_serializes_command_out() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();

        let command = Command::Out {
            title: "Out test".into(),
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

        assert_eq!(json["command"]["type"].as_str().unwrap(), "Out");
        assert_eq!(
            json["command"]["description"].as_str().unwrap(),
            "test for Command::Out"
        );

        let decoded: Entry = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, entry);
    }

    #[test]
    fn entry_serializes_command_log() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let command = Command::Log {
            title: "Log test".into(),
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

        assert_eq!(json["command"]["type"].as_str().unwrap(), "Log");
        assert_eq!(
            json["command"]["description"].as_str().unwrap(),
            "test for Command::Log"
        );

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

        assert_eq!(json["command"]["type"].as_str().unwrap(), "BreakStart");

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

        assert_eq!(json["command"]["type"].as_str().unwrap(), "BreakEnd");
        assert_eq!(
            json["command"]["description"].as_str().unwrap(),
            "test for Command::BreakEnd"
        );

        let decoded: Entry = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, entry);
    }
}
