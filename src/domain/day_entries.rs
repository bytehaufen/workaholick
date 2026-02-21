use std::{collections::BTreeMap, ops::Bound};

use chrono::NaiveDate;
use colored::Colorize;

use crate::domain::{EntryPatch, entry::EntryVersion};

use super::{ActivityKind, Command, Entry, EntryId};

#[derive(Debug, Clone, Copy)]
pub enum EntryMutation<'a> {
    Insert { patch: &'a EntryPatch },
    Remove { id: &'a EntryId },
    Update { id: &'a EntryId, patch: &'a EntryPatch },
}

#[derive(Debug, PartialEq)]
pub(crate) struct EntryValidationIndex {
    days: BTreeMap<NaiveDate, DayEntryIds>,
}

impl EntryValidationIndex {
    pub fn validate(
        entries: &[(&EntryId, &Entry)],
        mutation: EntryMutation<'_>,
    ) -> anyhow::Result<()> {
        let entries_by_day = Self::try_new(entries)?;

        match mutation {
            EntryMutation::Insert { patch } => entries_by_day.validate_insert_entry(entries, patch),
            EntryMutation::Remove { id } => entries_by_day.validate_remove_entry(entries, id),
            EntryMutation::Update { id, patch } => {
                entries_by_day.validate_update_entry(entries, id, patch)
            }
        }
    }

    pub fn validate_snapshot(entries: &[(&EntryId, &Entry)]) -> anyhow::Result<()> {
        Self::try_new(entries).map(|_| ())
    }

    fn validate_insert_entry(
        &self,
        entries: &[(&EntryId, &Entry)],
        patch: &EntryPatch,
    ) -> anyhow::Result<()> {
        let date = patch.timestamp().date_naive();
        let dummy = Entry::from_patch(EntryId::default(), EntryVersion::default(), patch.clone());

        let mut relevant_entries: Vec<(&EntryId, &Entry)> = Vec::new();

        let resolve = |id| -> anyhow::Result<(&EntryId, &Entry)> {
            Ok((id, Self::resolve_entry(entries, id)?))
        };

        // Previous day
        if let Some((_, prev_day)) = self.days.range(..date).next_back() {
            relevant_entries.extend(
                prev_day
                    .inner
                    .iter()
                    .map(resolve)
                    .collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
            );
        }

        // Current day (inclusive new one)
        if let Some(day) = self.days.get(&date) {
            relevant_entries.extend(
                day.inner
                    .iter()
                    .map(resolve)
                    .collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
            );
        }

        let dummy_id = EntryId::default();
        relevant_entries.push((&dummy_id, &dummy));

        // Next day
        if let Some((_, next_day)) =
            self.days.range((Bound::Excluded(date), Bound::Unbounded)).next()
        {
            relevant_entries.extend(
                next_day
                    .inner
                    .iter()
                    .map(resolve)
                    .collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
            );
        }

        Self::try_new(&relevant_entries)?;

        Ok(())
    }

    fn validate_remove_entry(
        &self,
        entries: &[(&EntryId, &Entry)],
        id: &EntryId,
    ) -> anyhow::Result<()> {
        let entry = Self::resolve_entry(entries, id)?;
        let date = entry.timestamp().date_naive();

        let mut relevant_entries: Vec<(&EntryId, &Entry)> = Vec::new();

        let resolve = |id| -> anyhow::Result<(&EntryId, &Entry)> {
            Ok((id, Self::resolve_entry(entries, id)?))
        };
        // Previous day
        if let Some((_, prev_day)) = self.days.range(..date).next_back() {
            relevant_entries.extend(
                prev_day
                    .inner
                    .iter()
                    .map(resolve)
                    .collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
            );
        }

        // Current day, excluding the removed one
        let day = self.days.get(&date).expect("current day should exist");
        relevant_entries.extend(
            day.inner
                .iter()
                .filter(|&entry_id| entry_id != id)
                .map(resolve)
                .collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
        );

        // Next day
        if let Some((_, next_day)) =
            self.days.range((Bound::Excluded(date), Bound::Unbounded)).next()
        {
            relevant_entries.extend(
                next_day
                    .inner
                    .iter()
                    .map(resolve)
                    .collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
            );
        }

        if !relevant_entries.is_empty() {
            Self::try_new(&relevant_entries)?;
        }

        Ok(())
    }

    fn validate_update_entry(
        &self,
        entries: &[(&EntryId, &Entry)],
        id: &EntryId,
        patch: &EntryPatch,
    ) -> anyhow::Result<()> {
        let entry = Self::resolve_entry(entries, id)?;
        let old_date = entry.date();
        let new_date = patch.date();

        if old_date != new_date {
            self.validate_remove_entry(entries, id)?;

            let mut entries_without_target: Vec<(&EntryId, &Entry)> = entries.to_vec();
            entries_without_target.retain(|(entry_id, _)| *entry_id != id);

            let by_day_without_target = Self::try_new(&entries_without_target)?;
            by_day_without_target.validate_insert_entry(&entries_without_target, patch)?;
        } else {
            let updated = Entry::from_patch(*id, EntryVersion::default(), patch.clone());
            let resolve = |entry_id| -> anyhow::Result<(&EntryId, &Entry)> {
                Ok((entry_id, Self::resolve_entry(entries, entry_id)?))
            };

            let mut relevant_entries: Vec<(&EntryId, &Entry)> = Vec::new();

            // Prev day
            if let Some((_, prev_day)) = self.days.range(..&patch.date()).next_back() {
                relevant_entries.extend(
                    prev_day
                        .inner
                        .iter()
                        .map(resolve)
                        .collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
                );
            }

            // Current day
            let day = self.days.get(&patch.date()).expect("current day should exist");
            relevant_entries.extend(
                day.inner
                    .iter()
                    .filter(|entry_id| *entry_id != id)
                    .map(resolve)
                    .collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
            );
            relevant_entries.push((id, &updated));

            // Next day
            if let Some((_, next_day)) =
                self.days.range((Bound::Excluded(patch.date()), Bound::Unbounded)).next()
            {
                relevant_entries.extend(
                    next_day
                        .inner
                        .iter()
                        .map(resolve)
                        .collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
                );
            }

            let _ = Self::try_new(&relevant_entries)?;
        }

        Ok(())
    }

    fn try_new(entries: &[(&EntryId, &Entry)]) -> anyhow::Result<Self> {
        let days = index_entries_by_date(entries)?;
        let dates: Vec<_> = days.keys().collect();

        // Iterate over dates to check that the date before ends with `Command::End`
        dates.windows(2).try_for_each(|window| {
            let date = window[0];
            let day = &days[date];

            let last_id = day
                .inner
                .last()
                .expect("invariant violated: indexed day must contain at least one entry");

            let last_entry = Self::resolve_entry(entries, last_id)?;

            match last_entry.command() {
                Command::End { .. } => {}
                _ => anyhow::bail!(" day {date} must end with 'End'"),
            }

            Ok(())
        })?;

        Ok(Self { days })
    }

    fn resolve_entry<'a>(
        entries: &'a [(&EntryId, &Entry)],
        id: &EntryId,
    ) -> anyhow::Result<&'a Entry> {
        let (_, entry) = entries
            .iter()
            .find(|(entry_id, _)| *entry_id == id)
            .ok_or_else(|| anyhow::anyhow!("entry {id} should exist during validation"))?;

        Ok(entry)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
struct DayEntryIds {
    inner: Vec<EntryId>,
}

impl TryFrom<&Vec<&Entry>> for DayEntryIds {
    type Error = DayEntryValidationError;

    fn try_from(entries: &Vec<&Entry>) -> Result<Self, DayEntryValidationError> {
        if entries.is_empty() {
            return Err(DayEntryValidationError {
                errors: vec!["entries must not be empty".into()],
            });
        }
        let first_date = entries[0].date();

        if !entries.iter().all(|entry| entry.date() == first_date) {
            return Err(DayEntryValidationError {
                errors: vec!["entries must all be of the same date".into()],
            });
        }

        validate_day_entries(entries)?;

        let ids = entries.iter().map(|entry| *entry.id()).collect();
        Ok(Self { inner: ids })
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Day entries validation failed with {} error(s):\n{}", errors.len(), errors.iter().map(|e| format!(" - {}", e)).collect::<Vec<_>>().join("\n"))]
struct DayEntryValidationError {
    errors: Vec<String>,
}

fn validate_day_entries(entries: &[&Entry]) -> Result<(), DayEntryValidationError> {
    #[derive(Clone, Copy)]
    enum State {
        Idle,
        Active(ActivityKind),
        Breaking(ActivityKind),
    }

    use State::*;

    let (_, errors) = entries.iter().fold((Idle, Vec::new()), |(mut state, mut errors), entry| {
        let result = match (state, entry.command()) {
            (Idle, Command::Start { kind, .. }) => Ok(Active(*kind)),
            (Idle, command) => {
                Err(format!("cannot {} while {}", format!("{command}").red(), "idle".yellow()))
            }

            (Active(_), Command::Log { .. }) => Ok(state),

            (Active(kind), Command::BreakStart) => Ok(Breaking(kind)),
            (Breaking(kind), Command::BreakEnd { .. }) => Ok(Active(kind)),

            (Active(_), Command::End { .. }) => Ok(Idle),

            (Active(activity_kind), Command::Start { kind }) => Err(format!(
                "cannot {} while already active ({})",
                format!("start {kind}").red(),
                activity_kind.to_string().yellow()
            )),

            (Active(_), Command::BreakEnd { .. }) => {
                Err(format!("cannot {} when {}", "end break".red(), "not breaking".yellow()))
            }

            (Breaking(_), Command::Start { .. } | Command::End { .. } | Command::Log { .. }) => {
                Err(format!(
                    "cannot {} while {}",
                    entry.command().to_string().red(),
                    "breaking".yellow()
                ))
            }

            (Breaking(_), Command::BreakStart) => {
                Err(format!("cannot {} while already {}", "start break".red(), "breaking".yellow()))
            }
        };

        match result {
            Err(error) => errors.push(format!("Entry {}: {}", entry.id(), error)),
            Ok(new_state) => state = new_state,
        }
        (state, errors)
    });

    if errors.is_empty() { Ok(()) } else { Err(DayEntryValidationError { errors }) }
}

fn index_entries_by_date(
    entries: &[(&EntryId, &Entry)],
) -> Result<BTreeMap<NaiveDate, DayEntryIds>, DayEntryValidationError> {
    let mut entries_by_date: BTreeMap<NaiveDate, Vec<&Entry>> =
        entries.iter().fold(BTreeMap::new(), |mut acc, (_entry_id, entry)| {
            acc.entry(entry.date()).or_default().push(entry);
            acc
        });

    entries_by_date
        .values_mut()
        .for_each(|entries| entries.sort_by_key(|entry| *entry.timestamp()));

    let (day_entries, errors) = entries_by_date.iter().fold(
        (BTreeMap::new(), Vec::new()),
        |(mut day_entries, mut errors), (date, entries)| {
            match DayEntryIds::try_from(entries) {
                Ok(day_entry) => {
                    day_entries.insert(*date, day_entry);
                }
                Err(validation_err) => errors.extend(validation_err.errors),
            };

            (day_entries, errors)
        },
    );

    if errors.is_empty() { Ok(day_entries) } else { Err(DayEntryValidationError { errors }) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::WorkContext;
    use chrono::{Local, TimeZone};
    use colored::control;
    use std::sync::Once;

    fn disable_colors_for_tests() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            control::set_override(false);
        });
    }

    fn ts(day: u32, hour: u32, min: u32) -> chrono::DateTime<Local> {
        Local.with_ymd_and_hms(2025, 1, day, hour, min, 0).unwrap()
    }

    fn entry(id: u64, ts: chrono::DateTime<Local>, cmd: Command) -> Entry {
        disable_colors_for_tests();
        Entry::new(id.into(), 1.into(), ts, cmd)
    }

    fn make_entry(id: u64, mins: u32, cmd: Command) -> Entry {
        entry(id, ts(10, 16, mins), cmd)
    }

    fn entries_at_same_day(cmds: &[Command]) -> Vec<Entry> {
        cmds.iter()
            .enumerate()
            .map(|(i, cmd)| make_entry((i + 1) as u64, (i as u32) * 5, cmd.clone()))
            .collect()
    }

    fn refs(entries: &[Entry]) -> Vec<&Entry> {
        entries.iter().collect()
    }

    fn id_refs(entries: &[Entry]) -> Vec<(&EntryId, &Entry)> {
        entries.iter().map(|e| (e.id(), e)).collect()
    }

    mod day_entries {
        use super::*;

        #[test]
        fn try_from_empty_returns_error() {
            let err = DayEntryIds::try_from(&Vec::new()).unwrap_err();
            assert_eq!(
                err.to_string(),
                "\
Day entries validation failed with 1 error(s):
 - entries must not be empty"
            );
        }

        #[test]
        fn try_from_mixed_dates_returns_error() {
            let entry1 = entry(1, ts(4, 16, 8), Command::default());
            let entry2 =
                entry(2, Local.with_ymd_and_hms(4025, 1, 4, 16, 9, 0).unwrap(), Command::default());

            let err = DayEntryIds::try_from(&vec![&entry1, &entry2]).unwrap_err();
            assert_eq!(
                err.to_string(),
                "\
Day entries validation failed with 1 error(s):
 - entries must all be of the same date"
            );
        }
    }

    mod index_entries_by_date_tests {
        use super::*;

        #[test]
        fn returns_empty_map_for_empty_input() {
            let input: Vec<(&EntryId, &Entry)> = Vec::new();
            let by_date = index_entries_by_date(&input).unwrap();
            assert!(by_date.is_empty());
        }

        #[test]
        fn partitions_entries_into_multiple_days() {
            let entry1 = entry(
                1,
                Local.with_ymd_and_hms(2026, 1, 4, 8, 0, 0).unwrap(),
                Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } },
            );
            let entry2 =
                entry(2, Local.with_ymd_and_hms(2026, 1, 4, 9, 0, 0).unwrap(), Command::BreakStart);
            let entry3 = entry(
                3,
                Local.with_ymd_and_hms(2026, 1, 5, 10, 0, 0).unwrap(),
                Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } },
            );

            let entries: Vec<(&EntryId, &Entry)> =
                vec![(entry1.id(), &entry1), (entry2.id(), &entry2), (entry3.id(), &entry3)];
            let by_date = index_entries_by_date(&entries).unwrap();

            assert_eq!(by_date.len(), 2);
            assert!(by_date.contains_key(&entry1.timestamp().date_naive()));
            assert!(by_date.contains_key(&entry3.timestamp().date_naive()));

            let day1 = by_date.get(&entry1.timestamp().date_naive()).unwrap();
            let day1_timestamps: Vec<_> = day1
                .inner
                .iter()
                .map(|id| *entries.iter().find(|(e_id, _)| *e_id == id).unwrap().1.timestamp())
                .collect();
            assert_eq!(day1_timestamps, vec![*entry1.timestamp(), *entry2.timestamp()]);

            let day2 = by_date.get(&entry3.timestamp().date_naive()).unwrap();
            let day2_timestamps: Vec<_> = day2
                .inner
                .iter()
                .map(|id| *entries.iter().find(|(e_id, _)| *e_id == id).unwrap().1.timestamp())
                .collect();
            assert_eq!(day2_timestamps, vec![*entry3.timestamp()]);
        }

        #[test]
        fn returns_collected_errors_message_snapshot() {
            let entries = [
                make_entry(1, 0, Command::Start { kind: Default::default() }),
                make_entry(2, 5, Command::Start { kind: Default::default() }),
                make_entry(3, 5, Command::Start { kind: Default::default() }),
                make_entry(5, 6, Command::Start { kind: Default::default() }),
                make_entry(6, 7, Command::Start { kind: Default::default() }),
                make_entry(7, 8, Command::Start { kind: Default::default() }),
            ];
            let entries: Vec<(&EntryId, &Entry)> = entries.iter().map(|e| (e.id(), e)).collect();

            let errors = index_entries_by_date(&entries).unwrap_err().to_string();
            assert_eq!(
                errors,
                "\
Day entries validation failed with 5 error(s):
 - Entry 2: cannot start work while already active (work)
 - Entry 3: cannot start work while already active (work)
 - Entry 5: cannot start work while already active (work)
 - Entry 6: cannot start work while already active (work)
 - Entry 7: cannot start work while already active (work)"
            );
        }
    }

    mod validate_day_entries_tests {
        use super::*;

        #[test]
        fn valid_full_day_returns_ok() {
            #[rustfmt::skip]
            let cmds = [
                Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } },
                Command::Log { title: "doing stuff".into(), description: None },
                Command::BreakStart,
                Command::BreakEnd { description: None },
                Command::Log { title: "doing other stuff".into(), description: None },
                Command::End { title: "done with all stuff".into(), description: None },
            ];

            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            assert!(validate_day_entries(&entries).is_ok());
        }

        #[test]
        fn multiple_checkin_checkout_cycles_returns_ok() {
            #[rustfmt::skip]
            let cmds = [
                Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } },
                Command::End { title: "id 2".into(), description: None },
                Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } },
                Command::End { title: "id 4".into(), description: None },
                Command::Start { kind: ActivityKind::Work { context: WorkContext::Office } },
                Command::End { title: "id 6".into(), description: None },
            ];

            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            assert!(validate_day_entries(&entries).is_ok());
        }

        #[test]
        fn given_active_start_returns_error() {
            let cmds = [
                Command::Start { kind: Default::default() },
                Command::Start { kind: Default::default() },
            ];
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_entries(&entries).unwrap_err();
            assert_eq!(
                err.to_string(),
                "\
Day entries validation failed with 1 error(s):
 - Entry 2: cannot start work while already active (work)"
            )
        }

        #[test]
        fn given_idle_end_returns_error_message() {
            let cmds = [Command::End { title: "".into(), description: None }];
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_entries(&entries).unwrap_err();
            assert_eq!(
                err.to_string(),
                "\
Day entries validation failed with 1 error(s):
 - Entry 1: cannot end while idle"
            );
        }

        #[test]
        fn given_idle_log_returns_error_message() {
            let cmds = [Command::Log { title: "".into(), description: None }];
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_entries(&entries).unwrap_err();
            assert_eq!(
                err.to_string(),
                "\
Day entries validation failed with 1 error(s):
 - Entry 1: cannot log while idle"
            )
        }

        #[test]
        fn given_idle_break_start_returns_error_message() {
            let cmds = [Command::BreakStart];
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_entries(&entries).unwrap_err();
            assert_eq!(
                err.to_string(),
                "\
Day entries validation failed with 1 error(s):
 - Entry 1: cannot start break while idle"
            );
        }

        #[test]
        fn given_active_break_end_returns_error_message() {
            let cmds = [
                Command::Start { kind: Default::default() },
                Command::BreakEnd { description: None },
            ];
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_entries(&entries).unwrap_err();
            assert_eq!(
                err.to_string(),
                "\
Day entries validation failed with 1 error(s):
 - Entry 2: cannot end break when not breaking"
            );
        }

        #[test]
        fn given_breaking_end_returns_error_message() {
            let cmds = [
                Command::Start { kind: Default::default() },
                Command::BreakStart,
                Command::End { title: "".into(), description: None },
            ];
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_entries(&entries).unwrap_err();
            assert_eq!(
                err.to_string(),
                "\
Day entries validation failed with 1 error(s):
 - Entry 3: cannot end while breaking"
            );
        }

        #[test]
        fn given_breaking_log_returns_error_message() {
            let cmds = [
                Command::Start { kind: Default::default() },
                Command::BreakStart,
                Command::Log { title: "".into(), description: None },
            ];
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_entries(&entries).unwrap_err();
            assert_eq!(
                err.to_string(),
                "\
Day entries validation failed with 1 error(s):
 - Entry 3: cannot log while breaking"
            );
        }

        #[test]
        fn given_breaking_break_start_returns_error_message() {
            let cmds = [
                Command::Start { kind: Default::default() },
                Command::BreakStart,
                Command::BreakStart,
            ];
            let entries = entries_at_same_day(&cmds);
            let entries = refs(&entries);

            let err = validate_day_entries(&entries).unwrap_err();
            assert_eq!(
                err.to_string(),
                "\
Day entries validation failed with 1 error(s):
 - Entry 3: cannot start break while already breaking"
            );
        }
    }

    mod entries_by_day_tests {
        use super::*;

        #[test]
        fn try_new_rejects_if_previous_day_missing_end() {
            let day1 = [entry(
                1,
                Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                Command::Start { kind: Default::default() },
            )];
            let day2 = [
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);

            let err = EntryValidationIndex::try_new(&entries).unwrap_err().to_string();
            assert!(err.contains("must end with 'End'"));
        }

        #[test]
        fn try_new_accepts_valid_days() {
            let ts1 = Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap();
            let ts2 = Local.with_ymd_and_hms(2025, 1, 10, 10, 0, 0).unwrap();
            let ts3 = Local.with_ymd_and_hms(2025, 1, 10, 11, 0, 0).unwrap();
            let day1 = [
                entry(1, ts1, Command::Start { kind: Default::default() }),
                entry(2, ts2, Command::Log { title: "".into(), description: None }),
                entry(3, ts3, Command::End { title: "".into(), description: None }),
            ];

            let ts4 = Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap();
            let ts5 = Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap();
            let day2 = [
                entry(4, ts4, Command::Start { kind: Default::default() }),
                entry(5, ts5, Command::Log { title: "".into(), description: None }),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();

            let mut expected: BTreeMap<NaiveDate, DayEntryIds> = BTreeMap::new();
            let day_entries_1 = DayEntryIds { inner: day1.iter().map(|e| *e.id()).collect() };
            let day_entries_2 = DayEntryIds { inner: day2.iter().map(|e| *e.id()).collect() };
            expected.insert(ts1.date_naive(), day_entries_1);
            expected.insert(ts4.date_naive(), day_entries_2);
            let expected = EntryValidationIndex { days: expected };

            assert_eq!(by_day, expected);
        }

        #[test]
        fn validate_insert_entry_rejects_when_it_breaks_previous_day_boundary() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 10, 0, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();

            let patch = EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 10, 11, 0, 0).unwrap(),
                Command::Log { title: "late log".into(), description: None },
            );

            let err = by_day.validate_insert_entry(&entries, &patch).unwrap_err().to_string();

            assert_eq!(
                err,
                "Day entries validation failed with 1 error(s):\n - Entry 0: cannot log while idle"
            );
        }

        #[test]
        fn validate_insert_entry_allows_valid_insert_in_same_day() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 10, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();

            let patch = EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 11, 9, 2, 0).unwrap(),
                Command::Log { title: "note".into(), description: None },
            );

            assert!(by_day.validate_insert_entry(&entries, &patch).is_ok());
        }

        #[test]
        fn validate_remove_entry_rejects_when_removal_makes_previous_day_invalid_for_next_day() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 5, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();

            let err = by_day.validate_remove_entry(&entries, day1[1].id()).unwrap_err().to_string();
            assert!(err.contains("must end with 'End'"));
        }

        #[test]
        fn validate_remove_entry_rejects_when_actual_day_gets_invalid() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 5, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();

            let err = by_day.validate_remove_entry(&entries, day2[0].id()).unwrap_err().to_string();
            assert!(err.contains("cannot end while idle"));
        }

        #[test]
        fn validate_remove_entry_allows_removing_last_entry_and_prunes_day_on_apply() {
            let only = [entry(
                1,
                Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                Command::Start { kind: Default::default() },
            )];

            let entries = id_refs(&only);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();

            by_day.validate_remove_entry(&entries, only[0].id()).unwrap();

            assert!(by_day.validate_remove_entry(&entries, only[0].id()).is_ok());
        }

        #[test]
        fn validate_remove_entry_allows_removing_valid() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 5, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 3, 0).unwrap(),
                    Command::Log { title: "".into(), description: None },
                ),
                entry(
                    5,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];
            let day3 = [
                entry(
                    6,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    7,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 5, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);
            all.extend_from_slice(&day3);

            let entries = id_refs(&all);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();

            by_day.validate_remove_entry(&entries, day2[1].id()).expect("expect valid entries");
        }

        #[test]
        fn validate_update_entry_allows_reordering_on_different_days() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    Command::Log { title: "a".into(), description: None },
                ),
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 10, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    5,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 5, 0).unwrap(),
                    Command::Log { title: "a".into(), description: None },
                ),
                entry(
                    6,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 10, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];
            let day3 = [
                entry(
                    7,
                    Local.with_ymd_and_hms(2025, 1, 13, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    8,
                    Local.with_ymd_and_hms(2025, 1, 13, 9, 5, 0).unwrap(),
                    Command::Log { title: "a".into(), description: None },
                ),
                entry(
                    9,
                    Local.with_ymd_and_hms(2025, 1, 13, 9, 10, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];
            let days: Vec<_> = vec![day1, day2.clone(), day3].into_iter().flatten().collect();

            let entries = id_refs(&days);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();

            let patch = EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 11, 9, 2, 0).unwrap(),
                Command::Log { title: "moved".into(), description: None },
            );

            let result = by_day.validate_update_entry(&entries, day2[1].id(), &patch);

            assert!(result.is_ok(), "expected Ok, but was Err: {:?}", result.unwrap_err());
        }

        #[test]
        fn validate_update_entry_allows_reordering_on_same_day() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    Command::Log { title: "a".into(), description: None },
                ),
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 10, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    5,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 5, 0).unwrap(),
                    Command::Log { title: "a".into(), description: None },
                ),
                entry(
                    6,
                    Local.with_ymd_and_hms(2025, 1, 12, 9, 10, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];
            let day3 = [
                entry(
                    7,
                    Local.with_ymd_and_hms(2025, 1, 13, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    8,
                    Local.with_ymd_and_hms(2025, 1, 13, 9, 5, 0).unwrap(),
                    Command::Log { title: "a".into(), description: None },
                ),
                entry(
                    9,
                    Local.with_ymd_and_hms(2025, 1, 13, 9, 10, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];
            let days: Vec<_> = vec![day1, day2.clone(), day3].into_iter().flatten().collect();

            let entries = id_refs(&days);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();

            let patch = EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 12, 9, 2, 0).unwrap(),
                Command::Log { title: "moved".into(), description: None },
            );

            let result = by_day.validate_update_entry(&entries, day2[1].id(), &patch);

            assert!(result.is_ok(), "expected Ok, but was Err: {:?}", result.unwrap_err());
        }

        #[test]
        fn validate_update_entry_rejects_invalid_same_day_command_change() {
            let day = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    Command::Log { title: "a".into(), description: None },
                ),
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 10, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];

            let entries = id_refs(&day);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();

            let patch = EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                Command::End { title: "nope".into(), description: None },
            );

            let err = by_day
                .validate_update_entry(&entries, day[1].id(), &patch)
                .unwrap_err()
                .to_string();
            assert!(err.contains("cannot end while idle"));
        }

        #[test]
        fn validate_update_entry_rejects_invalid_same_day_reordering() {
            let day = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    Command::Log { title: "a".into(), description: None },
                ),
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 10, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];

            let entries = id_refs(&day);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();

            let patch = EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 11, 8, 55, 0).unwrap(),
                Command::End { title: "".into(), description: None },
            );

            let err = by_day
                .validate_update_entry(&entries, day[2].id(), &patch)
                .unwrap_err()
                .to_string();
            assert!(err.contains("cannot end while idle"));
        }

        #[test]
        fn validate_update_entry_allows_move_to_new_day_when_old_day_becomes_empty() {
            let only = [entry(
                1,
                Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                Command::Start { kind: Default::default() },
            )];

            let entries = id_refs(&only);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();

            let patch = EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                Command::Start { kind: Default::default() },
            );

            let result = by_day.validate_update_entry(&entries, only[0].id(), &patch);
            assert!(result.is_ok(), "expected Ok, but was Err: {:?}", result.unwrap_err());
        }

        #[test]
        fn validate_update_entry_rejects_move_to_new_day_when_target_day_sequence_invalid() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 5, 0).unwrap(),
                    Command::Log { title: "a".into(), description: None },
                ),
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 10, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];
            let day2 = [
                entry(
                    4,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    5,
                    Local.with_ymd_and_hms(2025, 1, 11, 9, 5, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);
            all.extend_from_slice(&day2);

            let entries = id_refs(&all);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();

            let patch = EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 11, 9, 6, 0).unwrap(),
                Command::Log { title: "moved".into(), description: None },
            );

            let err = by_day
                .validate_update_entry(&entries, day1[1].id(), &patch)
                .unwrap_err()
                .to_string();
            assert!(err.contains("cannot log while idle"));
        }

        #[test]
        fn validate_update_entry_rejects_move_to_new_day_if_old_previous_day_not_closed() {
            let day1 = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 5, 0).unwrap(),
                    Command::Log { title: Default::default(), description: None },
                ),
                entry(
                    3,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 10, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];

            let mut all = Vec::new();
            all.extend_from_slice(&day1);

            let entries = id_refs(&all);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();

            let patch = EntryPatch::new(
                Local.with_ymd_and_hms(2025, 1, 11, 9, 10, 0).unwrap(),
                Command::Start { kind: Default::default() },
            );

            let err =
                by_day.validate_update_entry(&entries, &3.into(), &patch).unwrap_err().to_string();
            assert!(err.contains("must end with 'End'"));
        }

        #[test]
        fn validate_rejects_invalid_days() {
            let entry1 = entry(
                3,
                Local.with_ymd_and_hms(2025, 1, 10, 9, 10, 0).unwrap(),
                Command::End { title: "".into(), description: None },
            );

            let entries = vec![entry1.clone()];
            let entries = id_refs(&entries);

            let patch = EntryPatch::from_entry(&entry1);

            let err = EntryValidationIndex::validate(
                &entries,
                EntryMutation::Update { id: &3.into(), patch: &patch },
            )
            .unwrap_err()
            .to_string();

            assert!(err.contains("cannot end while idle"));
        }

        #[test]
        fn validate_remove_entry_rejects_missing_id() {
            let day = [
                entry(
                    1,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 0, 0).unwrap(),
                    Command::Start { kind: Default::default() },
                ),
                entry(
                    2,
                    Local.with_ymd_and_hms(2025, 1, 10, 9, 5, 0).unwrap(),
                    Command::End { title: "".into(), description: None },
                ),
            ];

            let entries = id_refs(&day);
            let by_day = EntryValidationIndex::try_new(&entries).unwrap();
            let missing_id: EntryId = 999.into();

            let err = by_day.validate_remove_entry(&entries, &missing_id).unwrap_err().to_string();

            assert!(err.contains("entry 999 should exist during validation"));
        }
    }
}
