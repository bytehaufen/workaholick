use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry<T> {
    id: u64,
    version: u16,
    timestamp: DateTime<Local>,
    date: NaiveDate,
    #[serde(flatten)]
    payload: T,
}

impl<T> Entry<T> {
    pub fn new(
        id: u64,
        version: u16,
        timestamp: DateTime<Local>,
        date: NaiveDate,
        payload: T,
    ) -> Entry<T> {
        Entry {
            id,
            version,
            timestamp,
            date,
            payload,
        }
    }
}
