mod absence;
mod cross;
mod error;
mod work;

use crate::domain::{AbsenceRecord, Record, WorkRecord, record_set::RecordSet};
use {absence::validate_absence_records, cross::validate_cross_rules, work::validate_work_records};

pub(crate) fn validate_records(records: &RecordSet) -> anyhow::Result<()> {
    let work_records: Vec<&WorkRecord> =
        records
            .iter()
            .filter_map(|record| {
                if let Record::Work(work_record) = record { Some(work_record) } else { None }
            })
            .collect();

    let absence_records: Vec<&AbsenceRecord> = records
        .iter()
        .filter_map(|record| {
            if let Record::Absence(absence_record) = record { Some(absence_record) } else { None }
        })
        .collect();

    validate_work_records(&work_records)?;
    validate_absence_records(&absence_records)?;
    validate_cross_rules(records)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::{
        domain::{
            WorkCommand, WorkContext, record_set::RecordSet, timeline_validation::validate_records,
        },
        test_support::{absence_record, work_record},
    };

    impl RecordSet {
        pub fn len(&self) -> usize {
            self.iter().collect::<Vec<_>>().len()
        }

        pub fn is_empty(&self) -> bool {
            self.iter().collect::<Vec<_>>().is_empty()
        }
    }

    #[test]
    fn validates_work_records() {
        let rec1 = work_record(1, 30, 6, 46, WorkCommand::Start { context: WorkContext::Home });
        let rec2 = work_record(
            2,
            30,
            6,
            50,
            WorkCommand::End { title: "refactor".to_string(), description: None },
        );
        let records = RecordSet::try_from_records(vec![rec1, rec2]).unwrap();

        assert_eq!(records.len(), 2);

        validate_records(&records).unwrap();
    }

    #[test]
    fn rejects_invalid_work_records() {
        let rec1 = work_record(1, 30, 6, 46, WorkCommand::BreakStart);
        let records = RecordSet::try_from_records(vec![rec1]).unwrap();
        let err = validate_records(&records).unwrap_err();

        assert_eq!(
            err.to_string(),
            "Day entries validation failed: Entry 1 - cannot start break while idle"
        );
    }

    #[test]
    fn rejects_full_day_with_half_absence() {
        let rec1 = absence_record(1, 31, AbsenceKind::Vacation, AbsencePortion::FullDay);
        let rec2 = absence_record(2, 31, AbsenceKind::Vacation, AbsencePortion::HalfDay);

        let records = RecordSet::try_from_records(vec![rec1, rec2]).unwrap();
        assert_eq!(records.len(), 2);

        let err = validate_records(&records).unwrap_err();
        assert_eq!(
            err.to_string(),
            "full-day absence cannot be combined with half-day absences for 2026-01-31"
        );
    }

    #[test]
    fn rejects_multiple_full_absences() {
        let rec1 = absence_record(1, 31, AbsenceKind::Holiday, AbsencePortion::FullDay);
        let rec2 = absence_record(2, 31, AbsenceKind::Vacation, AbsencePortion::FullDay);

        let records = RecordSet::try_from_records(vec![rec1, rec2]).unwrap();
        assert_eq!(records.len(), 2);

        let err = validate_records(&records).unwrap_err();
        assert_eq!(err.to_string(), "more than one full-day absence for 2026-01-31");
    }

    #[test]
    fn rejects_more_than_two_half_absences() {
        let rec1 = absence_record(1, 31, AbsenceKind::Sick, AbsencePortion::HalfDay);
        let rec2 = absence_record(2, 31, AbsenceKind::Sick, AbsencePortion::HalfDay);
        let rec3 = absence_record(3, 31, AbsenceKind::Sick, AbsencePortion::HalfDay);

        let records = RecordSet::try_from_records(vec![rec1, rec2, rec3]).unwrap();
        assert_eq!(records.len(), 3);

        let err = validate_records(&records).unwrap_err();
        assert_eq!(err.to_string(), "more than two half-day absences for 2026-01-31");
    }

    #[test]
    fn accepts_valid_absences() {
        let rec1 = absence_record(1, 1, AbsenceKind::Vacation, AbsencePortion::FullDay);
        let rec2 = absence_record(2, 2, AbsenceKind::Sick, AbsencePortion::HalfDay);
        let rec3 = absence_record(3, 2, AbsenceKind::Sick, AbsencePortion::HalfDay);
        let rec4 = absence_record(4, 3, AbsenceKind::Vacation, AbsencePortion::FullDay);
        let rec5 = absence_record(5, 4, AbsenceKind::Vacation, AbsencePortion::HalfDay);

        let records = RecordSet::try_from_records(vec![rec1, rec2, rec3, rec4, rec5]).unwrap();
        assert_eq!(records.len(), 5);

        validate_records(&records).unwrap();
    }

    #[test]
    fn start_when_previous_day_is_active() {
        let records = vec![
            work_record(10, 9, 0, 0, WorkCommand::Start { context: Default::default() }),
            work_record(2, 11, 9, 0, WorkCommand::Start { context: Default::default() }),
            work_record(3, 11, 9, 5, WorkCommand::End { title: "".into(), description: None }),
        ];

        let err = validate_records(&RecordSet::try_from_records(records).unwrap())
            .unwrap_err()
            .to_string();
        assert_eq!(
            err.to_string(),
            "Day entries validation failed: Entry 2 - cannot start home while already active (home)"
        );
    }

    #[test]
    fn allows_spanning_open_sessions_over_day_boundary() {
        let records = vec![
            work_record(0, 11, 5, 4, WorkCommand::Start { context: Default::default() }),
            work_record(1, 14, 20, 39, WorkCommand::End { title: "".into(), description: None }),
            work_record(2, 14, 21, 9, WorkCommand::Start { context: Default::default() }),
            work_record(3, 14, 21, 9, WorkCommand::End { title: "".into(), description: None }),
            work_record(4, 18, 6, 4, WorkCommand::Start { context: Default::default() }),
        ];

        let record_set = &RecordSet::try_from_records(records).unwrap();
        assert_eq!(record_set.len(), 5);
        validate_records(record_set).unwrap();
    }

    #[test]
    fn accepts_half_day_absence_with_work_on_same_day() {
        let records = vec![
            work_record(0, 13, 5, 13, WorkCommand::Start { context: Default::default() }),
            work_record(
                1,
                13,
                5,
                14,
                WorkCommand::End { title: "".to_string(), description: None },
            ),
            absence_record(2, 13, AbsenceKind::Sick, AbsencePortion::HalfDay),
        ];

        let record_set = &RecordSet::try_from_records(records).expect("should construct recordset");
        validate_records(record_set).expect("should validate cross rules")
    }

    #[test]
    fn rejects_full_day_absence_with_work_on_same_day() {
        let records = vec![
            work_record(0, 13, 5, 45, WorkCommand::Start { context: Default::default() }),
            work_record(
                1,
                13,
                5,
                46,
                WorkCommand::End { title: "".to_string(), description: None },
            ),
            absence_record(3, 13, AbsenceKind::Holiday, AbsencePortion::FullDay),
        ];

        let record_set = &RecordSet::try_from_records(records).expect("should construct recordset");
        let err = validate_records(record_set).unwrap_err();
        assert_eq!(
            err.to_string(),
            "full-day absence cannot coexist with work records for 2026-01-13"
        )
    }

    #[test]
    fn rejects_two_half_day_absence_with_work_on_same_day() {
        let records = vec![
            work_record(0, 13, 5, 45, WorkCommand::Start { context: Default::default() }),
            work_record(
                1,
                13,
                5,
                46,
                WorkCommand::End { title: "".to_string(), description: None },
            ),
            absence_record(3, 13, AbsenceKind::Holiday, AbsencePortion::HalfDay),
            absence_record(4, 13, AbsenceKind::Holiday, AbsencePortion::HalfDay),
        ];

        let record_set = &RecordSet::try_from_records(records).expect("should construct recordset");
        let err = validate_records(record_set).unwrap_err();
        assert_eq!(
            err.to_string(),
            "two half-day absences cannot coexist with work records for 2026-01-13"
        )
    }
}
