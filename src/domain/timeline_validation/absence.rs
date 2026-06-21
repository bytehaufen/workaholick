use std::collections::BTreeMap;

use crate::domain::AbsencePortion::{FullDay, HalfDay};
use crate::domain::AbsenceRecord;
use chrono::NaiveDate;

pub(super) fn validate_absence_records(absence_records: &[&AbsenceRecord]) -> anyhow::Result<()> {
    #[derive(Default)]
    struct DayAbsences {
        full: u32,
        half: u32,
    }

    let absences_by_day: BTreeMap<NaiveDate, DayAbsences> =
        absence_records.iter().fold(BTreeMap::new(), |mut by_day, absence_record| {
            match absence_record.portion() {
                FullDay => by_day.entry(*absence_record.date()).or_default().full += 1,
                HalfDay => by_day.entry(*absence_record.date()).or_default().half += 1,
            };

            by_day
        });

    absences_by_day.iter().try_for_each(|(date, day_absences)| match day_absences {
        DayAbsences { full, half } if *full > 0 && *half > 0 => {
            anyhow::bail!("full-day absence cannot be combined with half-day absences for {date}")
        }
        DayAbsences { full, .. } if *full > 1 => {
            anyhow::bail!("more than one full-day absence for {date}")
        }
        DayAbsences { half, .. } if *half > 2 => {
            anyhow::bail!("more than two half-day absences for {date}")
        }

        _ => Ok(()),
    })
}
