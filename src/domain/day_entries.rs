use std::collections::BTreeMap;

use chrono::NaiveDate;
use colored::Colorize;

use super::{ActivityKind, Command, Entry};

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

#[derive(Debug, thiserror::Error)]
#[error("Day entries validation failed with {} error(s): \n{}", errors.len(), errors.iter().map(|e| format!(" - {}", e)).collect::<Vec<_>>().join("\n"))]
pub struct DayEntriesValidationError {
    errors: Vec<String>,
}

// TODO: Move to separate file or not?
fn validate_day_entries(entries: &[&Entry]) -> anyhow::Result<()> {
    #[derive(Clone, Copy)]
    enum State {
        Idle,
        Active(ActivityKind),
        Breaking(ActivityKind),
    }

    use State::*;

    let mut state = Idle;
    let mut errors = Vec::new();

    for entry in entries {
        let result = match (state, entry.command()) {
            (Idle, Command::Start { kind, .. }) => Ok(Active(*kind)),
            (Idle, command) => Err(format!(
                "cannot {} while {}",
                format!("{command}").red(),
                "idle".yellow()
            )),

            (Active(_), Command::Log { .. }) => Ok(state),

            (Active(kind), Command::BreakStart) => Ok(Breaking(kind)),
            (Breaking(kind), Command::BreakEnd { .. }) => Ok(Active(kind)),

            (Active(_), Command::End { .. }) => Ok(Idle),

            (Active(activity_kind), Command::Start { kind }) => Err(format!(
                "cannot {} while already active ({})",
                format!("start {kind}").red(),
                activity_kind.to_string().yellow()
            )),

            (Active(_), Command::BreakEnd { .. }) => Err(format!(
                "cannot {} when {}",
                "end break".red(),
                "not breaking".yellow()
            )),

            (Breaking(_), Command::Start { .. } | Command::End { .. } | Command::Log { .. }) => {
                Err(format!(
                    "cannot '{}' while {}",
                    entry.command().to_string().red(),
                    "breaking".yellow()
                ))
            }

            (Breaking(_), Command::BreakStart) => Err(format!(
                "cannot {} while already {}",
                "start break".red(),
                "breaking".yellow()
            )),
        };

        match result {
            Ok(new_state) => state = new_state,
            Err(error) => errors.push(format!("Entry {}: {}", entry.id(), error)),
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(DayEntriesValidationError { errors }.into())
    }
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
    use super::*;
    use crate::domain::WorkContext;
    use chrono::{Local, TimeZone};

    #[ctor::ctor]
    fn init() {
        colored::control::set_override(false);
    }

    fn make_entry(id: u64, mins: u32, cmd: Command) -> Entry {
        let ts = Local.with_ymd_and_hms(2025, 1, 10, 16, mins, 0).unwrap();
        Entry::new(id.into(), 1.into(), ts, ts.date_naive(), cmd)
    }

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
    fn partitions_entries_into_multiple_days() {
        let ts1 = Local.with_ymd_and_hms(2026, 1, 4, 8, 0, 0).unwrap();
        let entry1 = Entry::new(
            0.into(),
            1.into(),
            ts1,
            ts1.date_naive(),
            Command::Start {
                kind: ActivityKind::Work {
                    context: WorkContext::Office,
                },
            },
        );

        let ts2 = Local.with_ymd_and_hms(2026, 1, 4, 9, 0, 0).unwrap();
        let entry2 = Entry::new(
            1.into(),
            1.into(),
            ts2,
            ts2.date_naive(),
            Command::BreakStart,
        );

        let ts3 = Local.with_ymd_and_hms(2026, 1, 5, 10, 0, 0).unwrap();
        let entry3 = Entry::new(
            2.into(),
            1.into(),
            ts3,
            ts3.date_naive(),
            Command::Start {
                kind: ActivityKind::Work {
                    context: WorkContext::Office,
                },
            },
        );

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

    #[test]
    fn accepts_valid_full_day() {
        let entries = [
            make_entry(
                1,
                0,
                Command::Start {
                    kind: ActivityKind::Work {
                        context: WorkContext::Office,
                    },
                },
            ),
            make_entry(
                2,
                5,
                Command::Log {
                    title: "doing stuff".into(),
                    description: None,
                },
            ),
            make_entry(3, 10, Command::BreakStart),
            make_entry(4, 15, Command::BreakEnd { description: None }),
            make_entry(
                5,
                20,
                Command::Log {
                    title: "doing other stuff".into(),
                    description: None,
                },
            ),
            make_entry(
                6,
                25,
                Command::End {
                    title: "done with all stuff".into(),
                    description: None,
                },
            ),
        ];

        let refs: Vec<&Entry> = entries.iter().collect();
        assert!(validate_day_entries(&refs).is_ok());
    }

    #[test]
    fn accepts_multiple_checkin_checkout_cycles() {
        let entries = [
            make_entry(
                1,
                0,
                Command::Start {
                    kind: ActivityKind::Work {
                        context: WorkContext::Office,
                    },
                },
            ),
            make_entry(
                2,
                5,
                Command::End {
                    title: "id 2".into(),
                    description: None,
                },
            ),
            make_entry(
                3,
                10,
                Command::Start {
                    kind: ActivityKind::Work {
                        context: WorkContext::Office,
                    },
                },
            ),
            make_entry(
                4,
                15,
                Command::End {
                    title: "id 4".into(),
                    description: None,
                },
            ),
            make_entry(
                5,
                20,
                Command::Start {
                    kind: ActivityKind::Work {
                        context: WorkContext::Office,
                    },
                },
            ),
            make_entry(
                6,
                25,
                Command::End {
                    title: "id 6".into(),
                    description: None,
                },
            ),
        ];

        let refs: Vec<&Entry> = entries.iter().collect();
        assert!(validate_day_entries(&refs).is_ok());
    }

    #[test]
    fn rejects_checkin_when_checked_in() {
        let entries = [
            make_entry(
                1,
                0,
                Command::Start {
                    kind: ActivityKind::Work {
                        context: WorkContext::Office,
                    },
                },
            ),
            make_entry(
                2,
                10,
                Command::Start {
                    kind: ActivityKind::Work {
                        context: WorkContext::Office,
                    },
                },
            ),
        ];
        let refs: Vec<&Entry> = entries.iter().collect();

        let err = validate_day_entries(&refs).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 2: cannot start work while already active (work)"
        )
    }

    #[test]
    fn rejects_checkout_when_not_checked_in() {
        let entries = [make_entry(
            1,
            0,
            Command::End {
                title: "".into(),
                description: None,
            },
        )];

        let refs: Vec<&Entry> = entries.iter().collect();
        let err = validate_day_entries(&refs).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 1: cannot end while idle"
        );
    }

    #[test]
    fn rejects_log_when_not_checked_in() {
        let entries = [make_entry(
            1,
            0,
            Command::Log {
                title: "".into(),
                description: None,
            },
        )];

        let refs: Vec<&Entry> = entries.iter().collect();
        let err = validate_day_entries(&refs).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 1: cannot log while idle"
        )
    }

    #[test]
    fn rejects_break_start_when_not_checked_in() {
        let entries = [make_entry(1, 0, Command::BreakStart)];

        let refs: Vec<&Entry> = entries.iter().collect();
        let err = validate_day_entries(&refs).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 1: cannot start break while idle"
        );
    }

    #[test]
    fn rejects_break_end_when_not_on_break() {
        let entries = [
            make_entry(
                1,
                0,
                Command::Start {
                    kind: ActivityKind::Work {
                        context: WorkContext::Office,
                    },
                },
            ),
            make_entry(2, 10, Command::BreakEnd { description: None }),
        ];
        let refs: Vec<&Entry> = entries.iter().collect();

        let err = validate_day_entries(&refs).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 2: cannot end break when not breaking"
        );
    }

    #[test]
    fn rejects_checkout_while_on_break() {
        let entries = [
            make_entry(
                1,
                0,
                Command::Start {
                    kind: ActivityKind::Work {
                        context: WorkContext::Office,
                    },
                },
            ),
            make_entry(2, 10, Command::BreakStart),
            make_entry(
                3,
                20,
                Command::End {
                    title: "".into(),
                    description: None,
                },
            ),
        ];
        let refs: Vec<&Entry> = entries.iter().collect();

        let err = validate_day_entries(&refs).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 3: cannot 'end' while breaking"
        );
    }

    #[test]
    fn rejects_log_while_on_break() {
        let entries = [
            make_entry(
                1,
                0,
                Command::Start {
                    kind: ActivityKind::Work {
                        context: WorkContext::Office,
                    },
                },
            ),
            make_entry(2, 10, Command::BreakStart),
            make_entry(
                3,
                20,
                Command::Log {
                    title: "".into(),
                    description: None,
                },
            ),
        ];
        let refs: Vec<&Entry> = entries.iter().collect();

        let err = validate_day_entries(&refs).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 3: cannot 'log' while breaking"
        );
    }
}
