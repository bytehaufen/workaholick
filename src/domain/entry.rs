use std::collections::BTreeMap;

use crate::domain::entry;

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

    pub fn date(&self) -> &NaiveDate {
        &self.date
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

/// A validated, per-day view over a set of entries.
///
/// Invariants guaranteed by `try_new`:
/// - `entries` is non-empty
/// - all entries have the same `NaiveDate` (`Entry::date()`)
/// - entries are sorted by `Entry::timestamp()` ascending
#[derive(Debug)]
pub struct DayEntries<'a>(Vec<&'a Entry>);

impl<'a> DayEntries<'a> {
    fn try_new(mut entries: Vec<&'a Entry>) -> anyhow::Result<Self> {
        if entries.is_empty() {
            anyhow::bail!("entries must not be empty");
        }

        let first_date = entries[0].date();

        if !entries.iter().all(|entry| *entry.date() == *first_date) {
            anyhow::bail!("entries must all be of the same date");
        }

        entries.sort_by(|a, b| a.timestamp().cmp(b.timestamp()));

        Ok(Self(entries))
    }

    pub fn iter(&self) -> impl Iterator<Item = &'a Entry> + '_ {
        self.0.iter().copied()
    }

    pub fn as_slice(&self) -> &[&'a Entry] {
        &self.0
    }
}

pub fn partition_by_date<'a>(
    entries: &[&'a Entry],
) -> anyhow::Result<BTreeMap<NaiveDate, DayEntries<'a>>> {
    let entries_by_date: BTreeMap<NaiveDate, Vec<&Entry>> =
        entries.iter().fold(BTreeMap::new(), |mut acc, entry| {
            acc.entry(*entry.date()).or_default().push(*entry);
            acc
        });

    entries_by_date
        .into_iter()
        .map(|(date, entries)| Ok((date, DayEntries::try_new(entries)?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    mod entry {
        use crate::domain::{Command, DayType};

        use super::super::*;
        use super::*;

        #[test]
        fn serializes_command_in() {
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
            assert_eq!(
                json["command"]["In"]["day_type"].as_str().unwrap(),
                "office"
            );

            let decoded: Entry = serde_json::from_value(json).unwrap();
            assert_eq!(decoded, entry);
        }

        #[test]
        fn serializes_command_out() {
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

            assert_eq!(
                json["command"]["Out"]["description"].as_str().unwrap(),
                "test for Command::Out"
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
            let entry = Entry::new(
                EntryId(1),
                EntryVersion(1),
                timestamp,
                timestamp.date_naive(),
                command,
            );

            let json = serde_json::to_value(&entry).unwrap();

            assert_eq!(
                json["command"]["Log"]["description"].as_str().unwrap(),
                "test for Command::Log"
            );

            let decoded: Entry = serde_json::from_value(json).unwrap();
            assert_eq!(decoded, entry);
        }

        #[test]
        fn serializes_command_break_start() {
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

            assert_eq!(json["command"].as_str().unwrap(), "BreakStart");

            let decoded: Entry = serde_json::from_value(json).unwrap();
            assert_eq!(decoded, entry);
        }

        #[test]
        fn serializes_command_break_end() {
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

            assert_eq!(
                json["command"]["BreakEnd"]["description"].as_str().unwrap(),
                "test for Command::BreakEnd"
            );

            let decoded: Entry = serde_json::from_value(json).unwrap();
            assert_eq!(decoded, entry);
        }
    }

    mod day_entries {
        use super::super::*;
        use super::*;

        #[test]
        fn try_new_rejects_empty_entries() {
            let err = DayEntries::try_new(Vec::new()).unwrap_err();
            assert_eq!(err.to_string(), "entries must not be empty");
        }

        #[test]
        fn try_new_rejects_not_unique_date() {
            let ts1 = Local.with_ymd_and_hms(2025, 1, 4, 16, 8, 0).unwrap();
            let entry1 = Entry::new(
                0.into(),
                1.into(),
                ts1,
                ts1.date_naive(),
                Command::default(),
            );
            let ts2 = Local.with_ymd_and_hms(4025, 1, 4, 16, 9, 0).unwrap();
            let entry2 = Entry::new(
                1.into(),
                1.into(),
                ts2,
                ts2.date_naive(),
                Command::default(),
            );

            let err = DayEntries::try_new(vec![&entry1, &entry2]).unwrap_err();
            assert_eq!(err.to_string(), "entries must all be of the same date");
        }

        #[test]
        fn try_new_sorts_entries_by_timestamp() {
            let ts1 = Local.with_ymd_and_hms(2025, 1, 4, 16, 13, 0).unwrap();
            let entry1 = Entry::new(
                0.into(),
                1.into(),
                ts1,
                ts1.date_naive(),
                Command::default(),
            );
            let ts2 = Local.with_ymd_and_hms(2025, 1, 4, 16, 9, 0).unwrap();
            let entry2 = Entry::new(
                1.into(),
                1.into(),
                ts2,
                ts2.date_naive(),
                Command::default(),
            );
            let ts3 = Local.with_ymd_and_hms(2025, 1, 4, 16, 8, 0).unwrap();
            let entry3 = Entry::new(
                2.into(),
                1.into(),
                ts3,
                ts3.date_naive(),
                Command::default(),
            );

            let day_entries = DayEntries::try_new(vec![&entry1, &entry2, &entry3]).unwrap();

            let timestamps: Vec<DateTime<Local>> =
                day_entries.iter().map(|entry| *entry.timestamp()).collect();
            assert_eq!(timestamps, vec![ts3, ts2, ts1]);
        }
    }

    mod partition_by_date {
        use super::super::*;
        use chrono::TimeZone;

        #[test]
        fn partitions_entries_into_multiple_days() {
            let ts1 = Local.with_ymd_and_hms(2026, 1, 4, 8, 0, 0).unwrap();
            let entry1 = Entry::new(
                1.into(),
                1.into(),
                ts1,
                ts1.date_naive(),
                Command::default(),
            );

            let ts2 = Local.with_ymd_and_hms(2026, 1, 5, 9, 0, 0).unwrap();
            let entry2 = Entry::new(
                2.into(),
                1.into(),
                ts2,
                ts2.date_naive(),
                Command::default(),
            );

            let ts3 = Local.with_ymd_and_hms(2026, 1, 4, 10, 0, 0).unwrap();
            let entry3 = Entry::new(
                3.into(),
                1.into(),
                ts3,
                ts3.date_naive(),
                Command::default(),
            );

            let input: Vec<&Entry> = vec![&entry1, &entry2, &entry3];

            let by_date = partition_by_date(&input).unwrap();

            assert_eq!(by_date.len(), 2);
            assert!(by_date.contains_key(&ts1.date_naive()));
            assert!(by_date.contains_key(&ts2.date_naive()));

            let day1 = by_date.get(&ts1.date_naive()).unwrap();
            let day1_timestamps: Vec<_> = day1.iter().map(|e| *e.timestamp()).collect();
            assert_eq!(day1_timestamps, vec![ts1, ts3]);

            let day2 = by_date.get(&ts2.date_naive()).unwrap();
            let day2_timestamps: Vec<_> = day2.iter().map(|e| *e.timestamp()).collect();
            assert_eq!(day2_timestamps, vec![ts2]);
        }

        #[test]
        fn returns_empty_map_for_empty_input() {
            let input: Vec<&Entry> = Vec::new();
            let by_date = partition_by_date(&input).unwrap();
            assert!(by_date.is_empty());
        }
    }
}
