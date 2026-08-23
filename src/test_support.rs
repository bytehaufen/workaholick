use chrono::{DateTime, Local, NaiveDate, TimeZone};

use crate::domain::{
    AbsenceKind, AbsencePortion, Record, RecordInput, WeeklySchedule, WorkCommand, WorkRecordInput,
};

pub(crate) fn ts(day: u32, hour: u32, min: u32) -> DateTime<Local> {
    Local.with_ymd_and_hms(2026, 1, day, hour, min, 0).unwrap()
}

pub(crate) fn date(day: u32) -> NaiveDate {
    ts(day, 0, 0).date_naive()
}

pub(crate) fn work_record(id: u64, day: u32, hour: u32, min: u32, command: WorkCommand) -> Record {
    Record::from_input(
        id.into(),
        RecordInput::Work(WorkRecordInput::new(ts(day, hour, min), command)),
    )
}

pub(crate) fn absence_record(
    id: u64,
    day: u32,
    kind: AbsenceKind,
    portion: AbsencePortion,
) -> Record {
    Record::from_input(
        id.into(),
        RecordInput::new_absence(ts(day, 0, 0).date_naive(), kind, portion, None),
    )
}

pub(crate) fn empty_schedule() -> WeeklySchedule {
    WeeklySchedule::new([0; 7])
}
