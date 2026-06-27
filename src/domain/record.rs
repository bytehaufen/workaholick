use std::{cmp::Ordering, fmt::Display};

use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::domain::{
    WorkCommand, WorkRecordInput,
    absence::{AbsenceRecord, AbsenceRecordInput},
    work::WorkRecord,
};

#[derive(
    Debug, PartialEq, Clone, Copy, PartialOrd, Eq, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct RecordId(u64);
impl Display for RecordId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

impl From<u64> for RecordId {
    fn from(id: u64) -> Self {
        RecordId(id)
    }
}

impl RecordId {
    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn incremented(&self) -> Self {
        Self::from(self.0 + 1)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Record {
    Work(WorkRecord),
    Absence(AbsenceRecord),
}

impl Record {
    pub fn sort_key(&self) -> RecordSortKey {
        match self {
            Record::Work(work_record) => {
                RecordSortKey::Work(*work_record.timestamp(), *work_record.id())
            }
            Record::Absence(absence_record) => {
                RecordSortKey::Absence(*absence_record.date(), *absence_record.id())
            }
        }
    }

    pub fn from_input(id: RecordId, input: RecordInput) -> Record {
        match input {
            RecordInput::Work(input) => Record::Work(WorkRecord::from_input(id, input)),
            RecordInput::Absence(input) => Record::Absence(AbsenceRecord::from_input(id, input)),
        }
    }

    pub fn id(&self) -> &RecordId {
        match self {
            Record::Work(record) => record.id(),
            Record::Absence(record) => record.id(),
        }
    }

    pub fn date(&self) -> NaiveDate {
        match self {
            Record::Work(work_record) => work_record.date(),
            Record::Absence(absence_record) => *absence_record.date(),
        }
    }

    pub fn as_work(&self) -> Option<&WorkRecord> {
        if let Record::Work(record) = self { Some(record) } else { None }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RecordInput {
    Work(WorkRecordInput),
    Absence(AbsenceRecordInput),
}

impl RecordInput {
    pub fn from_record(record: &Record) -> Self {
        match record {
            Record::Work(work_record) => {
                RecordInput::Work(WorkRecordInput::from_work_record(work_record))
            }
            Record::Absence(absence_record) => {
                RecordInput::Absence(AbsenceRecordInput::from_absence_record(absence_record))
            }
        }
    }

    pub fn new_work(timestamp: DateTime<Local>, command: WorkCommand) -> Self {
        Self::Work(WorkRecordInput::new(timestamp, command))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordSortKey {
    Work(DateTime<Local>, RecordId),
    Absence(NaiveDate, RecordId),
}

impl RecordSortKey {
    fn date(&self) -> NaiveDate {
        match self {
            RecordSortKey::Work(date_time, _) => date_time.date_naive(),
            RecordSortKey::Absence(naive_date, _) => *naive_date,
        }
    }

    fn variant_rank(&self) -> u8 {
        match self {
            RecordSortKey::Absence(..) => 0,
            RecordSortKey::Work(..) => 1,
        }
    }

    fn id(&self) -> &RecordId {
        match self {
            RecordSortKey::Work(_, record_id) => record_id,
            RecordSortKey::Absence(_, record_id) => record_id,
        }
    }
}

impl Ord for RecordSortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date()
            .cmp(&other.date())
            .then(self.variant_rank().cmp(&other.variant_rank()))
            .then_with(|| match (self, other) {
                (RecordSortKey::Work(date_time_1, _), RecordSortKey::Work(date_time_2, _)) => {
                    date_time_1.cmp(date_time_2)
                }
                _ => Ordering::Equal,
            })
            .then(self.id().cmp(other.id()))
    }
}

impl PartialOrd for RecordSortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::TimeZone;

    use crate::domain::{
        AbsenceKind, AbsencePortion, AbsenceRecordInput, WorkCommand, WorkContext, WorkRecordInput,
    };

    #[test]
    fn serializes_work_record_start() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let record = Record::from_input(
            RecordId(1),
            RecordInput::Work(WorkRecordInput::new(
                timestamp,
                WorkCommand::Start { context: WorkContext::Office },
            )),
        );

        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["type"].as_str().unwrap(), "work");
        assert_eq!(json["command"]["start"]["context"].as_str().unwrap(), "office");

        let decoded: Record = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.sort_key(), record.sort_key());
        assert_eq!(decoded.id(), record.id());
    }

    #[test]
    fn serializes_work_record_end() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let record = Record::from_input(
            RecordId(1),
            RecordInput::Work(WorkRecordInput::new(
                timestamp,
                WorkCommand::End {
                    title: "Out test".into(),
                    description: Some("test for Command::End".to_string()),
                },
            )),
        );
        let json = serde_json::to_value(&record).unwrap();

        assert_eq!(
            json["command"]["end"]["description"].as_str().unwrap(),
            "test for Command::End"
        );

        let decoded: Record = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.sort_key(), record.sort_key());
        assert_eq!(decoded.id(), record.id());
    }

    #[test]
    fn serializes_work_record_log() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let record = Record::from_input(
            RecordId(1),
            RecordInput::Work(WorkRecordInput::new(
                timestamp,
                WorkCommand::Log {
                    title: "Log test".into(),
                    description: Some("test for Command::Log".to_string()),
                },
            )),
        );

        let json = serde_json::to_value(&record).unwrap();

        assert_eq!(
            json["command"]["log"]["description"].as_str().unwrap(),
            "test for Command::Log"
        );

        let decoded: Record = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.sort_key(), record.sort_key());
        assert_eq!(decoded.id(), record.id());
    }

    #[test]
    fn serializes_work_record_break_start() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let record = Record::from_input(
            RecordId(1),
            RecordInput::Work(WorkRecordInput::new(timestamp, WorkCommand::BreakStart)),
        );

        let json = serde_json::to_value(&record).unwrap();

        assert_eq!(json["command"].as_str().unwrap(), "break_start");

        let decoded: Record = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.sort_key(), record.sort_key());
        assert_eq!(decoded.id(), record.id());
    }

    #[test]
    fn serializes_work_record_break_end() {
        let timestamp = Local.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let record = Record::from_input(
            RecordId(42),
            RecordInput::Work(WorkRecordInput::new(
                timestamp,
                WorkCommand::BreakEnd {
                    description: Some("test for Command::BreakEnd".to_string()),
                },
            )),
        );

        let json = serde_json::to_value(&record).unwrap();

        assert_eq!(
            json["command"]["break_end"]["description"].as_str().unwrap(),
            "test for Command::BreakEnd"
        );

        let decoded: Record = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.sort_key(), record.sort_key());
        assert_eq!(decoded.id(), record.id());
    }

    #[test]
    fn serializes_absence_record() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let record = Record::from_input(
            RecordId(7),
            RecordInput::Absence(AbsenceRecordInput::new(
                date,
                AbsenceKind::SickChild,
                AbsencePortion::HalfDay,
                Some("doctor".into()),
            )),
        );

        let json = serde_json::to_value(&record).unwrap();

        assert_eq!(json["type"].as_str().unwrap(), "absence");
        assert_eq!(json["kind"].as_str().unwrap(), "sick_child");
        assert_eq!(json["portion"].as_str().unwrap(), "half_day");
        assert_eq!(json["note"].as_str().unwrap(), "doctor");

        let decoded: Record = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.sort_key(), record.sort_key());
        assert_eq!(decoded.id(), record.id());
    }

    #[test]
    fn record_input_date_delegates_to_work_input() {
        let timestamp = Local.with_ymd_and_hms(2026, 2, 14, 14, 32, 14).unwrap();
        let input = RecordInput::Work(WorkRecordInput::new(
            timestamp,
            WorkCommand::Start { context: WorkContext::Home },
        ));
        let record = Record::from_input(0.into(), input);
        assert_eq!(record.date(), timestamp.date_naive());
    }

    #[test]
    fn record_input_date_delegates_to_absence_input() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let input = RecordInput::Absence(AbsenceRecordInput::new(
            date,
            AbsenceKind::Vacation,
            AbsencePortion::FullDay,
            None,
        ));
        let record = Record::from_input(0.into(), input);
        assert_eq!(record.date(), date);
    }

    #[test]
    fn from_record_round_trips_work_record_input() {
        let timestamp = Local.with_ymd_and_hms(2026, 2, 14, 14, 37, 14).unwrap();
        let record = Record::from_input(
            42.into(),
            RecordInput::Work(WorkRecordInput::new(
                timestamp,
                WorkCommand::Start { context: WorkContext::Office },
            )),
        );
        let input = RecordInput::from_record(&record);

        assert_eq!(
            input,
            RecordInput::Work(WorkRecordInput::new(
                timestamp,
                WorkCommand::Start { context: WorkContext::Office }
            ))
        );
    }

    #[test]
    fn from_record_round_trips_absence_record_input() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let record = Record::from_input(
            42.into(),
            RecordInput::Absence(AbsenceRecordInput::new(
                date,
                AbsenceKind::Holiday,
                AbsencePortion::FullDay,
                Some("regional".into()),
            )),
        );
        let input = RecordInput::from_record(&record);

        assert_eq!(
            input,
            RecordInput::Absence(AbsenceRecordInput::new(
                date,
                AbsenceKind::Holiday,
                AbsencePortion::FullDay,
                Some("regional".into())
            ))
        );
    }
}
