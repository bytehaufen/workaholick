use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::domain::{
    AbsencePortion::{self, FullDay, HalfDay},
    Record, RecordSet,
};

pub(super) fn validate_cross_rules(records: &RecordSet) -> anyhow::Result<()> {
    #[derive(Default)]
    struct DayView {
        has_work: bool,
        full_day_absence_count: usize,
        half_day_absence_count: usize,
    }

    impl DayView {
        fn add_work(&mut self) {
            self.has_work = true
        }
        fn add_absence(&mut self, portion: &AbsencePortion) {
            match portion {
                FullDay => self.full_day_absence_count += 1,
                HalfDay => self.half_day_absence_count += 1,
            }
        }
    }

    let days: BTreeMap<NaiveDate, DayView> =
        records.iter().fold(BTreeMap::new(), |mut days, record| {
            match record {
                Record::Work(work_record) => days.entry(work_record.date()).or_default().add_work(),
                Record::Absence(absence_record) => days
                    .entry(*absence_record.date())
                    .or_default()
                    .add_absence(absence_record.portion()),
            }
            days
        });

    days.iter().try_for_each(|(date, day_view)| match day_view {
        DayView { has_work, full_day_absence_count, .. }
            if *has_work && *full_day_absence_count > 0 =>
        {
            anyhow::bail!("full-day absence cannot coexist with work records for {date}")
        }
        DayView { has_work, half_day_absence_count, .. }
            if *has_work && *half_day_absence_count > 1 =>
        {
            anyhow::bail!("two half-day absences cannot coexist with work records for {date}")
        }
        _ => Ok(()),
    })?;

    Ok(())
}
