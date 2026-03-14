use std::{collections::BTreeMap, ops::Bound};

use chrono::NaiveDate;

use crate::domain::{Entry, EntryId, EntryPatch, entry::EntryVersion};

use super::EntryMutation;
use super::validation::{DayEntryInfo, DayEntryValidationError, State};

#[derive(Debug, PartialEq)]
pub(crate) struct EntryValidationIndex {
    pub(super) days: BTreeMap<NaiveDate, DayEntryInfo>,
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

    pub(super) fn validate_insert_entry(
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
                    .ids
                    .iter()
                    .map(resolve)
                    .collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
            );
        }

        // Current day (inclusive new one)
        if let Some(day) = self.days.get(&date) {
            relevant_entries.extend(
                day.ids.iter().map(resolve).collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
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
                    .ids
                    .iter()
                    .map(resolve)
                    .collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
            );
        }

        Self::try_new(&relevant_entries)?;

        Ok(())
    }

    pub(super) fn validate_remove_entry(
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
                    .ids
                    .iter()
                    .map(resolve)
                    .collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
            );
        }

        // Current day, excluding the removed one
        let day = self.days.get(&date).expect("current day should exist");
        relevant_entries.extend(
            day.ids
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
                    .ids
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

    pub(super) fn validate_update_entry(
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
                        .ids
                        .iter()
                        .map(resolve)
                        .collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
                );
            }

            // Current day
            let day = self.days.get(&patch.date()).expect("current day should exist");
            relevant_entries.extend(
                day.ids
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
                        .ids
                        .iter()
                        .map(resolve)
                        .collect::<anyhow::Result<Vec<(&EntryId, &Entry)>>>()?,
                );
            }

            let _ = Self::try_new(&relevant_entries)?;
        }

        Ok(())
    }

    pub(super) fn try_new(entries: &[(&EntryId, &Entry)]) -> anyhow::Result<Self> {
        let days = index_entries_by_date(entries)?;

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

pub(super) fn index_entries_by_date(
    entries: &[(&EntryId, &Entry)],
) -> Result<BTreeMap<NaiveDate, DayEntryInfo>, DayEntryValidationError> {
    let mut entries_by_date: BTreeMap<NaiveDate, Vec<&Entry>> =
        entries.iter().fold(BTreeMap::new(), |mut acc, (_entry_id, entry)| {
            acc.entry(entry.date()).or_default().push(entry);
            acc
        });

    entries_by_date.values_mut().for_each(|entries| entries.sort_by_key(|entry| entry.sort_key()));
    let initial_state = State::Idle;

    let (day_entries, _) = entries_by_date.iter().try_fold(
        (BTreeMap::new(), initial_state),
        |(mut day_entries, last_state), (date, entries)| {
            let day_entry = DayEntryInfo::try_new(entries, &last_state)?;

            let next_state = day_entry.end_state;
            day_entries.insert(*date, day_entry);

            Ok((day_entries, next_state))
        },
    )?;

    Ok(day_entries)
}
