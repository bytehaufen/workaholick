use std::fmt::Display;

use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

use super::{Command, serde_datetime};

#[derive(
    Debug, PartialEq, Clone, Copy, PartialOrd, Eq, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct EntryId(u64);
impl Display for EntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

impl From<u64> for EntryId {
    fn from(id: u64) -> Self {
        EntryId(id)
    }
}

impl EntryId {
    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn incremented(&self) -> Self {
        Self::from(self.0 + 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct EntryVersion(u8);

impl Default for EntryVersion {
    fn default() -> Self {
        // TODO: Think over - replace by Constant?
        Self(1)
    }
}

impl From<u8> for EntryVersion {
    fn from(id: u8) -> Self {
        EntryVersion(id)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    id: EntryId,
    version: EntryVersion,
    #[serde(with = "serde_datetime")]
    timestamp: DateTime<Local>,
    command: Command,
}

impl Entry {
    pub(crate) fn new(
        id: EntryId,
        version: EntryVersion,
        timestamp: DateTime<Local>,
        command: Command,
    ) -> Entry {
        Entry {
            id,
            version,
            timestamp,
            command,
        }
    }

    pub(crate) fn from_patch(id: EntryId, version: EntryVersion, new: EntryPatch) -> Entry {
        Self::new(id, version, new.timestamp, new.command)
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

    pub fn date(&self) -> NaiveDate {
        self.timestamp.date_naive()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntryPatch {
    #[serde(with = "serde_datetime")]
    timestamp: DateTime<Local>,
    command: Command,
}

impl EntryPatch {
    pub fn new(timestamp: DateTime<Local>, command: Command) -> Self {
        EntryPatch { timestamp, command }
    }

    pub fn timestamp(&self) -> DateTime<Local> {
        self.timestamp
    }

    pub fn date(&self) -> NaiveDate {
        self.timestamp.date_naive()
    }

    pub fn from_entry(entry: &Entry) -> Self {
        Self {
            timestamp: *entry.timestamp(),
            command: entry.command.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::TimeZone;

    use crate::domain::{ActivityKind, WorkContext};

    #[test]
    fn serializes_command_start() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let command = Command::Start {
            kind: ActivityKind::Work {
                context: WorkContext::Office,
            },
        };
        let entry = Entry::new(EntryId(1), EntryVersion(1), timestamp, command);

        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(
            json["command"]["start"]["kind"]["work"]["context"]
                .as_str()
                .unwrap(),
            "office"
        );

        let decoded: Entry = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, entry);
    }

    #[test]
    fn serializes_command_end() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();

        let command = Command::End {
            title: "Out test".into(),
            description: Some("test for Command::End".to_string()),
        };

        let entry = Entry::new(EntryId(1), EntryVersion(1), timestamp, command);
        let json = serde_json::to_value(&entry).unwrap();

        assert_eq!(
            json["command"]["end"]["description"].as_str().unwrap(),
            "test for Command::End"
        );

        let decoded: Entry = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, entry);
    }

    #[test]
    fn serializes_command_log() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let command = Command::Log {
            title: "Log test".into(),
            description: Some("test for Command::Log".to_string()),
        };
        let entry = Entry::new(EntryId(1), EntryVersion(1), timestamp, command);

        let json = serde_json::to_value(&entry).unwrap();

        assert_eq!(
            json["command"]["log"]["description"].as_str().unwrap(),
            "test for Command::Log"
        );

        let decoded: Entry = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, entry);
    }

    #[test]
    fn serializes_command_break_start() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let command = Command::BreakStart;
        let entry = Entry::new(EntryId(1), EntryVersion(1), timestamp, command);

        let json = serde_json::to_value(&entry).unwrap();

        assert_eq!(json["command"].as_str().unwrap(), "break_start");

        let decoded: Entry = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, entry);
    }

    #[test]
    fn serializes_command_break_end() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let command = Command::BreakEnd {
            description: Some("test for Command::BreakEnd".to_string()),
        };
        let entry: Entry = Entry::new(EntryId(42), EntryVersion(4), timestamp, command);

        let json = serde_json::to_value(&entry).unwrap();

        assert_eq!(
            json["command"]["break_end"]["description"]
                .as_str()
                .unwrap(),
            "test for Command::BreakEnd"
        );

        let decoded: Entry = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, entry);
    }
}
