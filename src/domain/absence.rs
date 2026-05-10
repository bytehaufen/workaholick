use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::domain::record::RecordId;

#[derive(
    Copy,
    Clone,
    Debug,
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
#[serde(rename_all = "snake_case")]
pub enum AbsenceKind {
    Vacation,
    Holiday,
    Sick,
    SickChild,
}

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
#[serde(rename_all = "snake_case")]
pub enum AbsencePortion {
    #[default]
    FullDay,
    HalfDay,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbsenceRecord {
    id: RecordId,
    date: NaiveDate,
    kind: AbsenceKind,
    portion: AbsencePortion,
    note: Option<String>,
}

impl AbsenceRecord {
    pub(crate) fn new(
        id: RecordId,
        date: NaiveDate,
        kind: AbsenceKind,
        portion: AbsencePortion,
        note: Option<String>,
    ) -> AbsenceRecord {
        AbsenceRecord { id, date, kind, portion, note }
    }

    pub(crate) fn from_input(id: RecordId, new: AbsenceRecordInput) -> AbsenceRecord {
        Self::new(id, new.date, new.kind, new.portion, new.note)
    }

    pub fn id(&self) -> &RecordId {
        &self.id
    }

    pub fn date(&self) -> &NaiveDate {
        &self.date
    }

    pub fn kind(&self) -> &AbsenceKind {
        &self.kind
    }

    pub fn portion(&self) -> &AbsencePortion {
        &self.portion
    }

    pub fn note(&self) -> &Option<String> {
        &self.note
    }

    pub fn sort_key(&self) -> (NaiveDate, RecordId) {
        (self.date, self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbsenceRecordInput {
    date: NaiveDate,
    kind: AbsenceKind,
    portion: AbsencePortion,
    note: Option<String>,
}

impl AbsenceRecordInput {
    pub fn new(
        date: NaiveDate,
        kind: AbsenceKind,
        portion: AbsencePortion,
        note: Option<String>,
    ) -> Self {
        AbsenceRecordInput { date, kind, portion, note }
    }

    pub fn date(&self) -> &NaiveDate {
        &self.date
    }

    pub fn portion(&self) -> &AbsencePortion {
        &self.portion
    }

    pub fn from_absence_record(record: &AbsenceRecord) -> Self {
        Self {
            date: *record.date(),
            kind: *record.kind(),
            portion: *record.portion(),
            note: record.note().clone(),
        }
    }
}
