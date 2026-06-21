use std::collections::BTreeMap;

use crate::domain::AbsencePortion::{FullDay, HalfDay};
use crate::domain::AbsenceRecord;
use crate::domain::timeline_validation::error::AbsenceValidationIssue;
use crate::domain::timeline_validation::error::TimelineValidationError;
use chrono::NaiveDate;

pub(super) fn validate_absence_records(
    absence_records: &[&AbsenceRecord],
) -> Result<(), TimelineValidationError> {
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
            Err(TimelineValidationError::absence(*date, AbsenceValidationIssue::FullDayWithHalfDay))
        }
        DayAbsences { full, .. } if *full > 1 => {
            Err(TimelineValidationError::absence(*date, AbsenceValidationIssue::MultipleFullDays))
        }
        DayAbsences { half, .. } if *half > 2 => Err(TimelineValidationError::absence(
            *date,
            AbsenceValidationIssue::MoreThanTwoHalfDays,
        )),

        _ => Ok(()),
    })
}
