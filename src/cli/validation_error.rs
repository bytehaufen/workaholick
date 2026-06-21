use anyhow::Error;
use colored::Colorize;
use std::backtrace::BacktraceStatus;

use crate::domain::{
    CommandLabel, DayRecordValidationError, DayRecordValidationIssue, TransitionReason,
};

pub fn render_error(err: &Error) -> String {
    let Some(issue) = find_validation_error(err).map(DayRecordValidationError::issue) else {
        return format!("{err:?}");
    };

    let mut rendered = render_colored_validation_error(issue);
    append_backtrace(err, &mut rendered);
    rendered
}

fn find_validation_error(err: &Error) -> Option<&DayRecordValidationError> {
    err.downcast_ref::<DayRecordValidationError>()
        .or_else(|| err.chain().find_map(|cause| cause.downcast_ref::<DayRecordValidationError>()))
}

fn render_colored_validation_error(issue: DayRecordValidationIssue) -> String {
    match issue {
        DayRecordValidationIssue::InvalidTransition { record_id, attempted, reason } => {
            match reason {
                TransitionReason::WhileIdle => format!(
                    "Day entries validation failed: Entry {record_id} - cannot {} while {}",
                    colorize_attempted(attempted),
                    "idle".yellow()
                ),
                TransitionReason::AlreadyActive(active_kind) => format!(
                    "Day entries validation failed: Entry {record_id} - cannot {} while already active ({})",
                    colorize_attempted(attempted),
                    active_kind.to_string().yellow()
                ),
                TransitionReason::NotBreaking => format!(
                    "Day entries validation failed: Entry {record_id} - cannot {} when {}",
                    colorize_attempted(attempted),
                    "not breaking".yellow()
                ),
                TransitionReason::WhileBreaking => format!(
                    "Day entries validation failed: Entry {record_id} - cannot {} while {}",
                    colorize_attempted(attempted),
                    "breaking".yellow()
                ),
                TransitionReason::AlreadyBreaking => format!(
                    "Day entries validation failed: Entry {record_id} - cannot {} while already {}",
                    colorize_attempted(attempted),
                    "breaking".yellow()
                ),
            }
        }
    }
}

fn colorize_attempted(attempted: CommandLabel) -> String {
    attempted.to_string().yellow().to_string()
}

fn append_backtrace(err: &Error, rendered: &mut String) {
    let backtrace = err.backtrace();
    if !matches!(backtrace.status(), BacktraceStatus::Captured) {
        return;
    }

    let mut formatted = backtrace.to_string();
    rendered.push_str("\n\n");

    if formatted.starts_with("stack backtrace:") {
        formatted.replace_range(0..1, "S");
    } else {
        rendered.push_str("Stack backtrace:\n");
    }

    formatted.truncate(formatted.trim_end().len());
    rendered.push_str(&formatted);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CommandLabel, Record, RecordId, RecordSet, WorkCommand, WorkContext, WorkRecord,
        validate_records,
    };
    use anyhow::{Context, anyhow};
    use chrono::{Local, TimeZone};
    use colored::control;
    use std::sync::{Mutex, OnceLock};

    fn with_color_override<T>(enabled: bool, test: impl FnOnce() -> T) -> T {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let lock = LOCK.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().unwrap();

        control::set_override(enabled);
        let result = test();
        control::unset_override();
        result
    }

    #[test]
    fn colorizes_transition_errors() {
        let text = with_color_override(true, || {
            render_colored_validation_error(DayRecordValidationIssue::InvalidTransition {
                record_id: RecordId::from(42),
                attempted: CommandLabel::Start(WorkContext::Office),
                reason: TransitionReason::AlreadyActive(WorkContext::Office),
            })
        });

        assert!(text.contains("\u{1b}["));
        assert!(text.contains("Entry 42 - cannot"));
    }

    #[test]
    fn render_error_passes_through_non_validation_errors() {
        let err = anyhow!("plain error");
        let rendered = render_error(&err);

        assert_eq!(rendered, format!("{err:?}"));
    }

    #[test]
    fn render_error_reformats_validation_errors() {
        let text = with_color_override(true, || {
            let ts = Local.with_ymd_and_hms(2026, 1, 12, 10, 0, 0).unwrap();
            let work_record = WorkRecord::new(
                1.into(),
                ts,
                WorkCommand::Log { title: "status".into(), description: None },
            );
            let records = vec![Record::Work(work_record)];

            let record_set = RecordSet::try_from_records(records).unwrap();
            let err = validate_records(&record_set).unwrap_err();

            render_error(&err)
        });

        assert!(text.contains("Day entries validation failed"));
        assert!(text.contains("Entry 1 - cannot"));
        assert!(text.contains("\u{1b}["));
        assert!(!text.contains("Caused by:"));
    }

    #[test]
    fn render_error_finds_validation_errors_inside_context() {
        let rendered = with_color_override(false, || {
            let ts = Local.with_ymd_and_hms(2026, 1, 12, 10, 0, 0).unwrap();
            let work_record = WorkRecord::new(
                1.into(),
                ts,
                WorkCommand::Log { title: "status".into(), description: None },
            );

            let err = validate_records(
                &RecordSet::try_from_records(vec![Record::Work(work_record)]).unwrap(),
            )
            .context("outer")
            .unwrap_err();

            assert!(find_validation_error(&err).is_some());
            render_error(&err)
        });

        assert!(rendered.contains("Day entries validation failed: Entry 1 - cannot"));
        assert!(rendered.contains("log"));
        assert!(rendered.contains("idle"));
        assert!(!rendered.contains("outer"));
        assert!(!rendered.contains("Caused by:"));
    }
}
