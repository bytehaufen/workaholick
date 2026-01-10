use std::collections::HashMap;

use chrono::{DateTime, Duration, FixedOffset, NaiveDate};

use crate::domain::{Command, DayEntries, DayType, Entry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveKind {
    Working,
    Break,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveState {
    pub kind: ActiveKind,
    pub since: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RangeKind {
    Day,
    Week,
    Month,
    Year,
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Totals {
    total_time: Duration,
    break_time: Duration,
    work_time: Duration,
}
impl Totals {
    fn new(entries: &DayEntries) -> Self {
        // TODO: Calculate
        Self {
            total_time: Duration::new(42, 42).unwrap(),
            break_time: Duration::new(42, 42).unwrap(),
            work_time: Duration::new(42, 42).unwrap(),
        }
    }
    pub fn total_time(&self) -> Duration {
        self.total_time
    }
    pub fn break_time(&self) -> Duration {
        self.break_time
    }
    pub fn work_time(&self) -> Duration {
        self.work_time
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SummaryRequest {
    Day(NaiveDate),
    Range {
        kind: RangeKind,
        start: NaiveDate,
        end: Option<NaiveDate>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Summary<'a> {
    Day {
        day_entries: DayEntries<'a>,
        totals: Option<Totals>,
    },
    // TODO: Fill range type
    Range,
}

pub fn summary<'a>(
    entries: &[&'a Entry],
    request: SummaryRequest,
) -> anyhow::Result<Option<Summary<'a>>> {
    match request {
        SummaryRequest::Day(date) => {
            let entries: Vec<&Entry> = entries
                .iter()
                .copied()
                .filter(|entry| *entry.date() == date)
                .collect();

            if entries.is_empty() {
                return Ok(None);
            }

            let day_type = entries.iter().find_map(|entry| match entry.command() {
                Command::In { day_type } => Some(*day_type),
                _ => None,
            });

            let day_entries = DayEntries::try_from(entries.clone())?;
            let totals = Totals::new(&day_entries);

            Ok(Some(Summary::Day {
                totals: Some(totals),
                day_entries,
            }))
        }

        // TODO: implement
        SummaryRequest::Range { kind, start, end } => {
            let in_range_inclusive = |entry_date: NaiveDate| {
                start <= entry_date && end.is_none_or(|date_end| entry_date <= date_end)
            };

            let date_entries: HashMap<NaiveDate, Vec<&Entry>> = entries
                .iter()
                .filter(|entry| in_range_inclusive(*entry.date()))
                .fold(HashMap::new(), |mut acc, entry| {
                    acc.entry(*entry.date()).or_default().push(entry);
                    acc
                });

            match kind {
                RangeKind::Day => anyhow::anyhow!("SummaryRequest::Range not implemented yet"),
                RangeKind::Week => anyhow::anyhow!("SummaryRequest::Range not implemented yet"),
                RangeKind::Month => anyhow::anyhow!("SummaryRequest::Range not implemented yet"),
                RangeKind::Year => anyhow::anyhow!("SummaryRequest::Range not implemented yet"),
                RangeKind::All => anyhow::anyhow!("SummaryRequest::Range not implemented yet"),
            };

            todo!()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::{Local, TimeZone};

    use crate::domain::{Command, DayEntries, DayType, Entry};

    #[ignore]
    #[test]
    fn calculates_totals() {
        let entries = &create_entries();
        let day_entries: DayEntries = entries.try_into().unwrap();

        let totals = Totals::new(&day_entries);

        //assert_eq!(totals.total_time().num_hours(), 9);
        //assert_eq!(totals.work_time().num_hours(), 8);
        //assert_eq!(totals.break_time().num_hours(), 1);
    }

    fn create_entries() -> Vec<Entry> {
        let mut entries: Vec<Entry> = Vec::new();

        let ts1 = Local.with_ymd_and_hms(2026, 1, 4, 8, 0, 0).unwrap();
        entries.push(Entry::new(
            1.into(),
            1.into(),
            ts1,
            ts1.date_naive(),
            Command::In {
                day_type: DayType::Office,
            },
        ));

        let ts2 = Local.with_ymd_and_hms(2026, 1, 4, 9, 0, 0).unwrap();
        entries.push(Entry::new(
            2.into(),
            1.into(),
            ts2,
            ts2.date_naive(),
            Command::Log {
                title: "log 1".into(),
                description: None,
            },
        ));

        let ts3 = Local.with_ymd_and_hms(2026, 1, 4, 10, 0, 0).unwrap();
        entries.push(Entry::new(
            2.into(),
            1.into(),
            ts3,
            ts3.date_naive(),
            Command::Log {
                title: "log 2".into(),
                description: None,
            },
        ));

        let ts4 = Local.with_ymd_and_hms(2026, 1, 4, 12, 0, 0).unwrap();
        entries.push(Entry::new(
            2.into(),
            1.into(),
            ts4,
            ts4.date_naive(),
            Command::BreakStart,
        ));

        let ts5 = Local.with_ymd_and_hms(2026, 1, 4, 13, 0, 0).unwrap();
        entries.push(Entry::new(
            2.into(),
            1.into(),
            ts5,
            ts5.date_naive(),
            Command::BreakEnd { description: None },
        ));

        let ts5 = Local.with_ymd_and_hms(2026, 1, 4, 14, 0, 0).unwrap();
        entries.push(Entry::new(
            2.into(),
            1.into(),
            ts5,
            ts5.date_naive(),
            Command::Log {
                title: "log 3".into(),
                description: None,
            },
        ));

        let ts6 = Local.with_ymd_and_hms(2026, 1, 4, 17, 0, 0).unwrap();
        entries.push(Entry::new(
            2.into(),
            1.into(),
            ts6,
            ts6.date_naive(),
            Command::Out {
                title: "out".into(),
                description: None,
            },
        ));

        entries
    }
}
