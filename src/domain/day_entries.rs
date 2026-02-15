use std::{collections::BTreeMap, ops::Bound};

use chrono::NaiveDate;
use colored::Colorize;

use crate::domain::{EntryPatch, entry::EntryVersion};

use super::{ActivityKind, Command, Entry, EntryId};

pub struct ValidatedAppend {
    patch: EntryPatch,
}
impl ValidatedAppend {
    pub fn get(&self) -> &EntryPatch {
        &self.patch
    }
}

pub struct ValidatedRemove {
    id: EntryId,
}
impl ValidatedRemove {
    pub fn get(&self) -> &EntryId {
        &self.id
    }
}

pub struct ValidatedUpdate {
    id: EntryId,
    patch: EntryPatch,
    new_index: usize,
}

#[derive(Debug)]
pub struct EntriesByDay {
    days: BTreeMap<NaiveDate, DayEntries>,
}

impl EntriesByDay {
    pub fn validate_append_entry(
        &self,
        entries: &[(&EntryId, &Entry)],
        patch: &EntryPatch,
    ) -> anyhow::Result<ValidatedAppend> {
        let date = patch.timestamp().date_naive();
        let dummy = Entry::from_patch(EntryId::default(), EntryVersion::default(), patch.clone());

        let mut relevant_entries: Vec<(&EntryId, &Entry)> = Vec::new();

        let resolve = |id| (id, Self::resolve_entry(entries, id));

        // Previous day
        if let Some((_, prev_day)) = self.days.range(..date).next_back() {
            relevant_entries.extend(prev_day.inner.iter().map(resolve));
        }

        // Current day (inclusive new one)
        if let Some(day) = self.days.get(&date) {
            relevant_entries.extend(day.inner.iter().map(resolve));
        }

        let dummy_id = EntryId::default();
        relevant_entries.push((&dummy_id, &dummy));

        // Next day
        if let Some((_, next_day)) = self
            .days
            .range((Bound::Excluded(date), Bound::Unbounded))
            .next()
        {
            relevant_entries.extend(next_day.inner.iter().map(resolve));
        }

        Self::try_new(&relevant_entries)?;

        Ok(ValidatedAppend {
            patch: patch.clone(),
        })
    }

    pub fn validate_remove_entry(
        &self,
        entries: &[(&EntryId, &Entry)],
        id: &EntryId,
    ) -> anyhow::Result<ValidatedRemove> {
        let entry = Self::resolve_entry(entries, id);
        let date = entry.timestamp().date_naive();

        let mut relevant_entries: Vec<(&EntryId, &Entry)> = Vec::new();

        let resolve = |id| (id, Self::resolve_entry(entries, id));
        // Previous day
        if let Some((_, prev_day)) = self.days.range(..date).next_back() {
            relevant_entries.extend(prev_day.inner.iter().map(resolve));
        }

        // Current day, excluding the removed one
        if let Some(day) = self.days.get(&date) {
            relevant_entries.extend(
                day.inner
                    .iter()
                    .filter(|&entry_id| entry_id != id)
                    .map(resolve),
            );
        }

        // Next day
        if let Some((_, next_day)) = self
            .days
            .range((Bound::Excluded(date), Bound::Unbounded))
            .next()
        {
            relevant_entries.extend(next_day.inner.iter().map(resolve));
        }

        if !relevant_entries.is_empty() {
            Self::try_new(&relevant_entries)?;
        }

        Ok(ValidatedRemove { id: *id })
    }

    pub fn validate_update_entry(
        &self,
        entries: &[(&EntryId, &Entry)],
        id: &EntryId,
        patch: &EntryPatch,
    ) -> anyhow::Result<ValidatedUpdate> {
        let entry = Self::resolve_entry(entries, id);
        let old_date = entry.date();
        let new_date = patch.date();

        let new_index = {
            let new_ts = patch.timestamp();
            let ids: Vec<EntryId> = self
                .days
                .get(&new_date)
                .map(|day| {
                    day.inner
                        .iter()
                        .copied()
                        .filter(|entry_id| entry_id != id)
                        .collect()
                })
                .unwrap_or_default();

            ids.iter()
                .position(|entry_id| *Self::resolve_entry(entries, entry_id).timestamp() > new_ts)
                .unwrap_or(ids.len())
        };

        if old_date != new_date {
            self.validate_remove_entry(entries, id)?;
            self.validate_append_entry(entries, patch)?;
        } else {
            let updated = Entry::from_patch(*id, EntryVersion::default(), patch.clone());
            let resolve = |entry_id| (entry_id, Self::resolve_entry(entries, entry_id));

            let mut relevant_entries: Vec<(&EntryId, &Entry)> = Vec::new();

            // Prev day
            if let Some((_, prev_day)) = self.days.range(..&patch.date()).next_back() {
                relevant_entries.extend(prev_day.inner.iter().map(resolve));
            }

            // Current day
            if let Some(day) = self.days.get(&patch.date()) {
                relevant_entries.extend(
                    day.inner
                        .iter()
                        .filter(|entry_id| *entry_id != id)
                        .map(resolve),
                );
            }
            relevant_entries.push((id, &updated));

            // Next day
            if let Some((_, next_day)) = self
                .days
                .range((Bound::Excluded(patch.date()), Bound::Unbounded))
                .next()
            {
                relevant_entries.extend(next_day.inner.iter().map(resolve));
            }

            let _ = Self::try_new(&relevant_entries)?;
        }

        Ok(ValidatedUpdate {
            id: *id,
            patch: patch.clone(),
            new_index,
        })
    }

    pub fn apply_append(&mut self, id: EntryId, patch: ValidatedAppend) {
        let date = patch.get().timestamp().date_naive();
        let day_entries = self.days.entry(date).or_default();
        day_entries.inner.push(id);
    }

    pub fn apply_remove(&mut self, id: ValidatedRemove) {
        let id = *id.get();
        self.days.retain(|_, day| {
            day.inner.retain(|entry_id| *entry_id != id);
            !day.inner.is_empty()
        });
    }

    pub fn apply_update(&mut self, update: ValidatedUpdate) {
        let ValidatedUpdate {
            id,
            patch,
            new_index,
        } = update;
        let new_date = patch.date();

        self.days.retain(|_, day| {
            day.inner.retain(|entry_id| *entry_id != id);
            !day.inner.is_empty()
        });

        let day_entries = self.days.entry(new_date).or_default();
        day_entries.inner.retain(|entry_id| *entry_id != id);

        let insert_pos = new_index.min(day_entries.inner.len());
        day_entries.inner.insert(insert_pos, id);
    }

    pub(crate) fn try_new(entries: &[(&EntryId, &Entry)]) -> anyhow::Result<Self> {
        let days = partition_by_date(entries)?;
        let dates: Vec<_> = days.keys().collect();

        // Iterate over dates to check that the date before ends with `Command::End`
        dates.windows(2).try_for_each(|window| {
            let date = window[0];
            let day = &days[date];

            if let Some(last_id) = day.inner.last() {
                let last_entry = Self::resolve_entry(entries, last_id);

                match last_entry.command() {
                    Command::End { .. } => {}
                    _ => anyhow::bail!(" day {date} must end with 'End'"),
                }
            }

            Ok(())
        })?;

        Ok(Self { days })
    }

    fn resolve_entry<'a>(entries: &'a [(&EntryId, &Entry)], id: &EntryId) -> &'a Entry {
        let (_, entry) = entries
            .iter()
            .find(|(entry_id, _)| *entry_id == id)
            .expect("entry should exist");

        entry
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DayEntries {
    inner: Vec<EntryId>,
}

impl TryFrom<&Vec<&Entry>> for DayEntries {
    type Error = DayEntriesValidationError;

    fn try_from(entries: &Vec<&Entry>) -> Result<Self, DayEntriesValidationError> {
        if entries.is_empty() {
            return Err(DayEntriesValidationError {
                errors: vec!["entries must not be empty".into()],
            });
        }
        let first_date = entries[0].date();

        if !entries.iter().all(|entry| entry.date() == first_date) {
            return Err(DayEntriesValidationError {
                errors: vec!["entries must all be of the same date".into()],
            });
        }

        validate_day_entries(entries)?;

        let ids = entries.iter().map(|entry| *entry.id()).collect();
        Ok(Self { inner: ids })
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Day entries validation failed with {} error(s): \n{}", errors.len(), errors.iter().map(|e| format!(" - {}", e)).collect::<Vec<_>>().join("\n"))]
pub struct DayEntriesValidationError {
    errors: Vec<String>,
}

fn validate_day_entries(entries: &[&Entry]) -> Result<(), DayEntriesValidationError> {
    #[derive(Clone, Copy)]
    enum State {
        Idle,
        Active(ActivityKind),
        Breaking(ActivityKind),
    }

    use State::*;

    let (_, errors) = entries
        .iter()
        .fold((Idle, Vec::new()), |(mut state, mut errors), entry| {
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

                (
                    Breaking(_),
                    Command::Start { .. } | Command::End { .. } | Command::Log { .. },
                ) => Err(format!(
                    "cannot '{}' while {}",
                    entry.command().to_string().red(),
                    "breaking".yellow()
                )),

                (Breaking(_), Command::BreakStart) => Err(format!(
                    "cannot {} while already {}",
                    "start break".red(),
                    "breaking".yellow()
                )),
            };

            match result {
                Err(error) => errors.push(format!("Entry {}: {}", entry.id(), error)),
                Ok(new_state) => state = new_state,
            }
            (state, errors)
        });

    if errors.is_empty() {
        Ok(())
    } else {
        Err(DayEntriesValidationError { errors })
    }
}

// impl<'a> TryFrom<Vec<&'a Entry>> for DayEntries<'a> {
//     type Error = anyhow::Error;
//     fn try_from(entries: Vec<&'a Entry>) -> anyhow::Result<Self> {
//         Self::try_new(entries)
//     }
// }

pub fn partition_by_date(
    entries: &[(&EntryId, &Entry)],
) -> Result<BTreeMap<NaiveDate, DayEntries>, DayEntriesValidationError> {
    let entries_by_date: BTreeMap<NaiveDate, Vec<&Entry>> =
        entries
            .iter()
            .fold(BTreeMap::new(), |mut acc, (_entry_id, entry)| {
                acc.entry(entry.date()).or_default().push(entry);
                acc
            });

    let (day_entries, errors) = entries_by_date.iter().fold(
        (BTreeMap::new(), Vec::new()),
        |(mut day_entries, mut errors), (date, entries)| {
            match DayEntries::try_from(entries) {
                Ok(day_entry) => {
                    day_entries.insert(*date, day_entry);
                }
                Err(validation_err) => errors.extend(validation_err.errors),
            };

            (day_entries, errors)
        },
    );

    if errors.is_empty() {
        Ok(day_entries)
    } else {
        Err(DayEntriesValidationError { errors })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::WorkContext;
    use chrono::{Local, TimeZone};

    fn make_entry(id: u64, mins: u32, cmd: Command) -> Entry {
        let ts = Local.with_ymd_and_hms(2025, 1, 10, 16, mins, 0).unwrap();
        Entry::new(id.into(), 1.into(), ts, cmd)
    }

    #[test]
    fn try_new_rejects_empty_entries() {
        let err = DayEntries::try_from(&Vec::new()).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - entries must not be empty"
        );
    }

    #[test]
    fn try_new_rejects_not_unique_date() {
        let ts1 = Local.with_ymd_and_hms(2025, 1, 4, 16, 8, 0).unwrap();
        let entry1 = Entry::new(0.into(), 1.into(), ts1, Command::default());
        let ts2 = Local.with_ymd_and_hms(4025, 1, 4, 16, 9, 0).unwrap();
        let entry2 = Entry::new(1.into(), 1.into(), ts2, Command::default());

        let err = DayEntries::try_from(&vec![&entry1, &entry2]).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - entries must all be of the same date"
        );
    }

    #[test]
    fn partitions_entries_into_multiple_days() {
        let ts1 = Local.with_ymd_and_hms(2026, 1, 4, 8, 0, 0).unwrap();
        let entry1 = Entry::new(
            0.into(),
            1.into(),
            ts1,
            Command::Start {
                kind: ActivityKind::Work {
                    context: WorkContext::Office,
                },
            },
        );

        let ts2 = Local.with_ymd_and_hms(2026, 1, 4, 9, 0, 0).unwrap();
        let entry2 = Entry::new(1.into(), 1.into(), ts2, Command::BreakStart);

        let ts3 = Local.with_ymd_and_hms(2026, 1, 5, 10, 0, 0).unwrap();
        let entry3 = Entry::new(
            2.into(),
            1.into(),
            ts3,
            Command::Start {
                kind: ActivityKind::Work {
                    context: WorkContext::Office,
                },
            },
        );

        let entries: Vec<(&EntryId, &Entry)> = vec![
            (entry1.id(), &entry1),
            (entry2.id(), &entry2),
            (entry3.id(), &entry3),
        ];

        let by_date = partition_by_date(&entries).unwrap();

        assert_eq!(by_date.len(), 2);
        assert!(by_date.contains_key(&ts1.date_naive()));
        assert!(by_date.contains_key(&ts2.date_naive()));

        let day1 = by_date.get(&ts1.date_naive()).unwrap();

        let day1_timestamps: Vec<_> = day1
            .inner
            .iter()
            .map(|id| {
                *entries
                    .iter()
                    .find(|(e_id, _)| *e_id == id)
                    .unwrap()
                    .1
                    .timestamp()
            })
            .collect();
        assert_eq!(day1_timestamps, vec![ts1, ts2]);

        let day2 = by_date.get(&ts3.date_naive()).unwrap();
        let day2_timestamps: Vec<_> = day2
            .inner
            .iter()
            .map(|id| {
                *entries
                    .iter()
                    .find(|(e_id, _)| *e_id == id)
                    .unwrap()
                    .1
                    .timestamp()
            })
            .collect();
        assert_eq!(day2_timestamps, vec![ts3]);
    }

    #[test]
    fn returns_empty_map_for_empty_input() {
        let input: Vec<(&EntryId, &Entry)> = Vec::new();
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

        let entries: Vec<&Entry> = entries.iter().collect();

        assert!(validate_day_entries(&entries).is_ok());
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

        let entries: Vec<&Entry> = entries.iter().collect();

        assert!(validate_day_entries(&entries).is_ok());
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

        let entries: Vec<&Entry> = entries.iter().collect();

        let err = validate_day_entries(&entries).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 2: cannot \u{1b}[31mstart work\u{1b}[0m while already active (\u{1b}[33mwork\u{1b}[0m)"
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

        let entries: Vec<&Entry> = entries.iter().collect();

        let err = validate_day_entries(&entries).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 1: cannot \u{1b}[31mend\u{1b}[0m while \u{1b}[33midle\u{1b}[0m"
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

        let entries: Vec<&Entry> = entries.iter().collect();

        let err = validate_day_entries(&entries).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 1: cannot \u{1b}[31mlog\u{1b}[0m while \u{1b}[33midle\u{1b}[0m"
        )
    }

    #[test]
    fn rejects_break_start_when_not_checked_in() {
        let entries = [make_entry(1, 0, Command::BreakStart)];
        let entries: Vec<&Entry> = entries.iter().collect();

        let err = validate_day_entries(&entries).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 1: cannot \u{1b}[31mstart break\u{1b}[0m while \u{1b}[33midle\u{1b}[0m"
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

        let entries: Vec<&Entry> = entries.iter().collect();

        let err = validate_day_entries(&entries).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 2: cannot \u{1b}[31mend break\u{1b}[0m when \u{1b}[33mnot breaking\u{1b}[0m"
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

        let entries: Vec<&Entry> = entries.iter().collect();

        let err = validate_day_entries(&entries).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 3: cannot '\u{1b}[31mend\u{1b}[0m' while \u{1b}[33mbreaking\u{1b}[0m"
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

        let entries: Vec<&Entry> = entries.iter().collect();

        let err = validate_day_entries(&entries).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 3: cannot '\u{1b}[31mlog\u{1b}[0m' while \u{1b}[33mbreaking\u{1b}[0m"
        );
    }

    #[test]
    fn rejects_break_while_on_break() {
        let entries = [
            make_entry(
                1,
                0,
                Command::Start {
                    kind: Default::default(),
                },
            ),
            make_entry(2, 1, Command::BreakStart {}),
            make_entry(2, 2, Command::BreakStart {}),
        ];

        let entries: Vec<&Entry> = entries.iter().collect();

        let err = validate_day_entries(&entries).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed with 1 error(s): \n - Entry 2: cannot \u{1b}[31mstart break\u{1b}[0m while already \u{1b}[33mbreaking\u{1b}[0m"
        );
    }
}
