use std::{collections::BTreeMap, fmt::Display};

use super::{Command, date_time_codec};
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Copy, PartialOrd, Eq, Ord, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq)]
pub struct DayEntries<'a>(Vec<&'a Entry>);

impl<'a> DayEntries<'a> {
    fn try_new(entries: Vec<&'a Entry>) -> anyhow::Result<Self> {
        if entries.is_empty() {
            anyhow::bail!("entries must not be empty");
        }

        let first_date = entries[0].date();

        if !entries.iter().all(|entry| *entry.date() == *first_date) {
            anyhow::bail!("entries must all be of the same date");
        }

        validate_day_entries(&entries)?;

        Ok(Self(entries))
    }

    pub fn iter(&self) -> impl Iterator<Item = &'a Entry> + '_ {
        self.0.iter().copied()
    }

    pub fn as_slice(&self) -> &[&'a Entry] {
        &self.0
    }
}

fn validate_day_entries(entries: &[&Entry]) -> anyhow::Result<()> {
    #[derive(Debug, Clone, Copy)]
    enum WorkState {
        CheckedIn,
        CheckedOut,
        OnBreak,
    }
    impl Display for WorkState {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(match self {
                CheckedIn => "checked in",
                CheckedOut => "checked out",
                OnBreak => "on break",
            })
        }
        // add code here
    }
    use WorkState::*;

    entries
        .iter()
        .try_fold(CheckedOut, |state, entry| match (state, entry.command()) {
            (CheckedOut, Command::In { .. }) => Ok(CheckedIn),
            (CheckedIn, Command::Out { .. }) => Ok(CheckedOut),
            (CheckedIn, Command::Log { .. }) => Ok(CheckedIn),
            (CheckedIn, Command::BreakStart) => Ok(OnBreak),
            (OnBreak, Command::BreakEnd { .. }) => Ok(CheckedIn),

            (CheckedIn | OnBreak, Command::In { .. }) => {
                anyhow::bail!(
                    "cannot check in when {state} (entry #{},, timestamp: {}",
                    entry.id,
                    entry.timestamp
                )
            }
            (CheckedOut | OnBreak, Command::Out { .. }) => {
                anyhow::bail!(
                    "cannot check out when {state} (entry #{},, timestamp: {}",
                    entry.id,
                    entry.timestamp
                )
            }
            (CheckedOut | OnBreak, Command::Log { .. }) => {
                anyhow::bail!(
                    "cannot log when {state} (entry #{},, timestamp: {}",
                    entry.id,
                    entry.timestamp
                )
            }
            (CheckedOut | OnBreak, Command::BreakStart) => {
                anyhow::bail!(
                    "cannot start break when {state} (entry #{},, timestamp: {}",
                    entry.id,
                    entry.timestamp
                )
            }
            (CheckedIn | CheckedOut, Command::BreakEnd { .. }) => {
                anyhow::bail!(
                    "cannot end break when {state} (entry #{},, timestamp: {}",
                    entry.id,
                    entry.timestamp
                )
            }
        })?;
    Ok(())
}

impl<'a> TryFrom<&'a Vec<Entry>> for DayEntries<'a> {
    type Error = anyhow::Error;

    fn try_from(entries: &'a Vec<Entry>) -> anyhow::Result<Self> {
        Self::try_new(entries.iter().collect())
    }
}

impl<'a> TryFrom<Vec<&'a Entry>> for DayEntries<'a> {
    type Error = anyhow::Error;
    fn try_from(entries: Vec<&'a Entry>) -> anyhow::Result<Self> {
        Self::try_new(entries)
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
            #[rustfmt::skip]
            let entry = Entry::new( EntryId(1), EntryVersion(1), timestamp, timestamp.date_naive(), command);

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

            #[rustfmt::skip]
            let entry = Entry::new( EntryId(1), EntryVersion(1), timestamp, timestamp.date_naive(), command);
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
            #[rustfmt::skip]
            let entry = Entry::new( EntryId(1), EntryVersion(1), timestamp, timestamp.date_naive(), command);

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
            #[rustfmt::skip]
            let entry = Entry::new( EntryId(1), EntryVersion(1), timestamp, timestamp.date_naive(), command);

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
            #[rustfmt::skip]
            let entry: Entry = Entry::new( EntryId(42), EntryVersion(4), timestamp, timestamp.date_naive(), command);

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
            #[rustfmt::skip]
            let entry1 = Entry::new( 0.into(), 1.into(), ts1, ts1.date_naive(), Command::default());
            let ts2 = Local.with_ymd_and_hms(4025, 1, 4, 16, 9, 0).unwrap();
            #[rustfmt::skip]
            let entry2 = Entry::new( 1.into(), 1.into(), ts2, ts2.date_naive(), Command::default());

            let err = DayEntries::try_new(vec![&entry1, &entry2]).unwrap_err();
            assert_eq!(err.to_string(), "entries must all be of the same date");
        }
    }

    mod partition_by_date {
        use crate::domain::DayType;

        use super::super::*;
        use chrono::TimeZone;

        #[test]
        fn partitions_entries_into_multiple_days() {
            let ts1 = Local.with_ymd_and_hms(2026, 1, 4, 8, 0, 0).unwrap();
            #[rustfmt::skip]
            let entry1 = Entry::new( 0.into(), 1.into(), ts1, ts1.date_naive(), Command::In { day_type: DayType::Travel });

            let ts2 = Local.with_ymd_and_hms(2026, 1, 4, 9, 0, 0).unwrap();
            #[rustfmt::skip]
            let entry2 = Entry::new( 1.into(), 1.into(), ts2, ts2.date_naive(), Command::BreakStart);

            let ts3 = Local.with_ymd_and_hms(2026, 1, 5, 10, 0, 0).unwrap();
            #[rustfmt::skip]
            let entry3 = Entry::new( 2.into(), 1.into(), ts3, ts3.date_naive(), Command::In { day_type: DayType::Home });

            let input: Vec<&Entry> = vec![&entry1, &entry2, &entry3];

            let by_date = partition_by_date(&input).unwrap();

            assert_eq!(by_date.len(), 2);
            assert!(by_date.contains_key(&ts1.date_naive()));
            assert!(by_date.contains_key(&ts2.date_naive()));

            let day1 = by_date.get(&ts1.date_naive()).unwrap();
            let day1_timestamps: Vec<_> = day1.iter().map(|e| *e.timestamp()).collect();
            assert_eq!(day1_timestamps, vec![ts1, ts2]);

            let day2 = by_date.get(&ts3.date_naive()).unwrap();
            let day2_timestamps: Vec<_> = day2.iter().map(|e| *e.timestamp()).collect();
            assert_eq!(day2_timestamps, vec![ts3]);
        }

        #[test]
        fn returns_empty_map_for_empty_input() {
            let input: Vec<&Entry> = Vec::new();
            let by_date = partition_by_date(&input).unwrap();
            assert!(by_date.is_empty());
        }
    }

    mod validate_day_entries_tests {
        use chrono::TimeZone;

        use crate::domain::DayType;

        use super::super::*;

        fn make_entry(id: u64, mins: u32, cmd: Command) -> Entry {
            let ts = Local.with_ymd_and_hms(2025, 1, 10, 16, mins, 0).unwrap();
            Entry::new(id.into(), 1.into(), ts, ts.date_naive(), cmd)
        }

        #[test]
        fn accepts_valid_full_day() {
            #[rustfmt::skip]
            let entries = [
                make_entry( 1, 0, Command::In { day_type: DayType::Office }),
                make_entry( 2, 5, Command::Log { title: "doing stuff".into(), description: None }),
                make_entry( 3, 10, Command::BreakStart),
                make_entry( 4, 15, Command::BreakEnd { description: None }),
                make_entry( 5, 20, Command::Log { title: "doing other stuff".into(), description: None }),
                make_entry( 6, 25, Command::Out { title: "done with all stuff".into(), description: None }),
            ];

            let refs: Vec<&Entry> = entries.iter().collect();
            assert!(validate_day_entries(&refs).is_ok());
        }

        #[test]
        fn accepts_multiple_checkin_checkout_cycles() {
            #[rustfmt::skip]
            let entries  = [
                make_entry( 1, 0, Command::In { day_type: DayType::Office }),
                make_entry( 2, 5, Command::Out { title: "id 2".into(), description: None }),
                make_entry( 3, 10, Command::In { day_type: DayType::Travel }),
                make_entry( 4, 15, Command::Out { title: "id 4".into(), description: None }),
                make_entry( 5, 20, Command::In { day_type: DayType::Home }),
                make_entry( 6, 25, Command::Out { title: "id 6".into(), description: None }),
            ];

            let refs: Vec<&Entry> = entries.iter().collect();
            assert!(validate_day_entries(&refs).is_ok());
        }

        #[test]
        fn rejects_checkin_when_checked_in() {
            #[rustfmt::skip]
            let entries = [
                make_entry(1, 0, Command::In { day_type: DayType::Office }),
                make_entry(2, 10, Command::In { day_type: DayType::Home })
            ];
            let refs: Vec<&Entry> = entries.iter().collect();

            let err = validate_day_entries(&refs).unwrap_err();
            assert!(
                err.to_string()
                    .starts_with("cannot check in when checked in")
            );
        }

        #[test]
        fn rejects_checkout_when_not_checked_in() {
            #[rustfmt::skip]
            let entries = [ make_entry(1, 0, Command::Out { title: "".into(), description: None }) ];

            let refs: Vec<&Entry> = entries.iter().collect();
            let err = validate_day_entries(&refs).unwrap_err();
            assert!(
                err.to_string()
                    .starts_with("cannot check out when checked out")
            );
        }

        #[test]
        fn rejects_log_when_not_checked_in() {
            #[rustfmt::skip]
            let entries = [ make_entry(1, 0, Command::Log { title: "".into(), description: None }) ];

            let refs: Vec<&Entry> = entries.iter().collect();
            let err = validate_day_entries(&refs).unwrap_err();
            assert!(err.to_string().starts_with("cannot log when checked out"))
        }

        #[test]
        fn rejects_break_start_when_not_checked_in() {
            let entries = [make_entry(1, 0, Command::BreakStart)];

            let refs: Vec<&Entry> = entries.iter().collect();
            let err = validate_day_entries(&refs).unwrap_err();
            assert!(
                err.to_string()
                    .starts_with("cannot start break when checked out")
            );
        }

        #[test]
        fn rejects_break_end_when_not_on_break() {
            #[rustfmt::skip]
            let entries = [
                make_entry(1, 0, Command::In { day_type: DayType::Office }),
                make_entry(2, 10, Command::BreakEnd { description: None }),
            ];
            let refs: Vec<&Entry> = entries.iter().collect();

            let err = validate_day_entries(&refs).unwrap_err();
            assert!(
                err.to_string()
                    .starts_with("cannot end break when checked in")
            );
        }

        #[test]
        fn rejects_checkout_while_on_break() {
            #[rustfmt::skip]
            let entries = [
                make_entry(1, 0, Command::In { day_type: DayType::Office }),
                make_entry(2, 10, Command::BreakStart),
                make_entry(3, 20, Command::Out { title: "".into(), description: None }),
            ];
            let refs: Vec<&Entry> = entries.iter().collect();

            let err = validate_day_entries(&refs).unwrap_err();
            assert!(
                err.to_string()
                    .starts_with("cannot check out when on break")
            );
        }

        #[test]
        fn rejects_log_while_on_break() {
            #[rustfmt::skip]
            let entries = [
                make_entry(1, 0, Command::In { day_type: DayType::Office }),
                make_entry(2, 10, Command::BreakStart),
                make_entry(3, 20, Command::Log { title: "".into(), description: None }),
            ];
            let refs: Vec<&Entry> = entries.iter().collect();

            let err = validate_day_entries(&refs).unwrap_err();
            assert!(err.to_string().starts_with("cannot log when on break"));
        }
    }
}
