use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::domain::record::RecordId;
use crate::domain::serde_datetime;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, strum::Display, strum::EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum WorkCommand {
    #[strum(to_string = "start")]
    Start { context: WorkContext },
    #[strum(to_string = "end")]
    End { title: String, description: Option<String> },
    #[strum(to_string = "log")]
    Log { title: String, description: Option<String> },
    #[strum(to_string = "start break")]
    BreakStart,
    #[strum(to_string = "end break")]
    BreakEnd { description: Option<String> },
}

impl Default for WorkCommand {
    fn default() -> Self {
        Self::Log { title: String::new(), description: None }
    }
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
pub enum WorkContext {
    #[default]
    Home,
    Office,
    Travel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkRecord {
    id: RecordId,
    #[serde(with = "serde_datetime")]
    timestamp: DateTime<Local>,
    command: WorkCommand,
}

impl WorkRecord {
    pub(crate) fn new(
        id: RecordId,
        timestamp: DateTime<Local>,
        command: WorkCommand,
    ) -> WorkRecord {
        WorkRecord { id, timestamp, command }
    }

    pub(crate) fn from_input(id: RecordId, new: WorkRecordInput) -> WorkRecord {
        Self::new(id, new.timestamp, new.command)
    }

    pub fn id(&self) -> &RecordId {
        &self.id
    }

    pub fn command(&self) -> &WorkCommand {
        &self.command
    }

    pub fn timestamp(&self) -> &DateTime<Local> {
        &self.timestamp
    }

    pub fn date(&self) -> NaiveDate {
        self.timestamp.date_naive()
    }

    pub fn sort_key(&self) -> (DateTime<Local>, RecordId) {
        (self.timestamp, self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkRecordInput {
    #[serde(with = "serde_datetime")]
    timestamp: DateTime<Local>,
    command: WorkCommand,
}

impl WorkRecordInput {
    pub fn new(timestamp: DateTime<Local>, command: WorkCommand) -> Self {
        WorkRecordInput { timestamp, command }
    }

    #[allow(unused)]
    pub fn timestamp(&self) -> DateTime<Local> {
        self.timestamp
    }

    pub fn date(&self) -> NaiveDate {
        self.timestamp.date_naive()
    }

    pub fn from_work_record(record: &WorkRecord) -> Self {
        Self { timestamp: *record.timestamp(), command: record.command.clone() }
    }
}
