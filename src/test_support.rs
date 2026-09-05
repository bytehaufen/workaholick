use std::path::PathBuf;

use chrono::{DateTime, Local, NaiveDate, TimeDelta, TimeZone};

use crate::{
    config::{CliConfig, Config},
    domain::{
        AbsenceKind, AbsencePortion, Record, RecordInput, TimeBreakdown, TimeCategory,
        WeeklySchedule, WorkCommand, WorkRecordInput,
    },
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

pub(crate) fn test_config() -> Config {
    Config {
        store_path: PathBuf::from("/tmp/workaholick-test.jsonl"),
        schedule: Default::default(),
        cli: CliConfig::default(),
    }
}

pub(crate) fn active_time(breakdown: &TimeBreakdown, kind: TimeCategory) -> TimeDelta {
    match breakdown.iter().find(|(candidate, _)| *candidate == kind) {
        Some((_, bucket)) => bucket.active_time(),
        None => TimeDelta::zero(),
    }
}

pub(crate) fn break_time(breakdown: &TimeBreakdown, kind: TimeCategory) -> TimeDelta {
    match breakdown.iter().find(|(candidate, _)| *candidate == kind) {
        Some((_, bucket)) => bucket.break_time(),
        None => TimeDelta::zero(),
    }
}
