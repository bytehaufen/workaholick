use super::Command;
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    id: u64,
    version: u16,
    timestamp: DateTime<Local>,
    date: NaiveDate,
    #[serde(flatten)]
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
