use crate::domain::{Record, WorkRecord, record_set::RecordSet};

use super::day_entries;

pub(crate) fn validate_records(records: &RecordSet) -> anyhow::Result<()> {
    // Validate work records
    let work_records: Vec<&WorkRecord> =
        records
            .iter()
            .filter_map(|record| {
                if let Record::Work(work_record) = record { Some(work_record) } else { None }
            })
            .collect();

    validate_work_records(&work_records)?;

    Ok(())
}

fn validate_work_records(work_records: &[&WorkRecord]) -> anyhow::Result<()> {
    // TODO: Extract later
    // TODO: Rename `index_records_by_date` as it validates and indexes, also remove the result
    let _ = day_entries::index_records_by_date(work_records)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            WorkCommand, WorkContext, record_set::RecordSet, timeline_validation::validate_records,
        },
        test_support::work_record,
    };

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
}
