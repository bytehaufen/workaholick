use std::collections::HashMap;

use chrono::{DateTime, Duration, FixedOffset, NaiveDate};

use crate::domain::{Command, DayType, Entry};

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
pub enum SummaryRequest {
    Day(NaiveDate),
    Range {
        kind: RangeKind,
        start: NaiveDate,
        end: Option<NaiveDate>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Totals {
    pub worked: Duration,
    pub breaks: Duration,
    pub net: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Summary<'a> {
    Day {
        entries: Vec<&'a Entry>,
        day_type: Option<DayType>,
        totals: Option<Totals>,
    },
    // TODO: Fill range type
    Range,
}

pub fn summary<'a>(entries: &[&'a Entry], request: SummaryRequest) -> Option<Summary<'a>> {
    match request {
        SummaryRequest::Day(date) => {
            let entries: Vec<&Entry> = entries
                .iter()
                .copied()
                .filter(|entry| *entry.date() == date)
                .collect();

            if entries.is_empty() {
                return None;
            }

            let day_type = entries.iter().find_map(|entry| match entry.command() {
                Command::In { day_type } => Some(*day_type),
                _ => None,
            });

            Some(Summary::Day {
                entries,
                day_type,
                totals: Some(Totals {
                    worked: Duration::new(42, 42).unwrap(),
                    breaks: Duration::new(42, 42).unwrap(),
                    net: Duration::new(42, 42).unwrap(),
                }),
            })
        }

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
                RangeKind::Day => println!("{date_entries:#?}"),
                RangeKind::Week => todo!(),
                RangeKind::Month => todo!(),
                RangeKind::Year => todo!(),
                RangeKind::All => todo!(),
            }

            todo!()
        }
    }
}
