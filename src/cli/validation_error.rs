use anyhow::Error;
use colored::Colorize;
use std::backtrace::BacktraceStatus;

use crate::domain::{
    CommandLabel, DayEntryValidationError, DayEntryValidationIssue, TransitionReason,
};

pub fn render_error(err: &Error) -> String {
    let Some(issue) = find_validation_error(err).map(DayEntryValidationError::issue) else {
        return format!("{err:?}");
    };

    let mut rendered = render_colored_validation_error(issue);
    append_backtrace(err, &mut rendered);
    rendered
}

fn find_validation_error(err: &Error) -> Option<&DayEntryValidationError> {
    err.downcast_ref::<DayEntryValidationError>()
        .or_else(|| err.chain().find_map(|cause| cause.downcast_ref::<DayEntryValidationError>()))
}

fn render_colored_validation_error(issue: DayEntryValidationIssue) -> String {
    match issue {
        DayEntryValidationIssue::EmptyEntries | DayEntryValidationIssue::MixedDates => {
            format!("Day entries validation failed: {issue}")
        }
        DayEntryValidationIssue::InvalidTransition { entry_id, attempted, reason } => {
            match reason {
                TransitionReason::WhileIdle => format!(
                    "Day entries validation failed: Entry {entry_id} - cannot {} while {}",
                    colorize_attempted(attempted),
                    "idle".yellow()
                ),
                TransitionReason::AlreadyActive(active_kind) => format!(
                    "Day entries validation failed: Entry {entry_id} - cannot {} while already active ({})",
                    colorize_attempted(attempted),
                    active_kind.to_string().yellow()
                ),
                TransitionReason::NotBreaking => format!(
                    "Day entries validation failed: Entry {entry_id} - cannot {} when {}",
                    colorize_attempted(attempted),
                    "not breaking".yellow()
                ),
                TransitionReason::WhileBreaking => format!(
                    "Day entries validation failed: Entry {entry_id} - cannot {} while {}",
                    colorize_attempted(attempted),
                    "breaking".yellow()
                ),
                TransitionReason::AlreadyBreaking => format!(
                    "Day entries validation failed: Entry {entry_id} - cannot {} while already {}",
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
        ActivityKind, Command, CommandLabel, Entry, EntryId, EntryValidationIndex,
    };
    use anyhow::anyhow;
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
    fn keeps_structural_errors_plain() {
        let text = render_colored_validation_error(DayEntryValidationIssue::EmptyEntries);

        assert_eq!(text, "Day entries validation failed: entries must not be empty");
    }

    #[test]
    fn colorizes_transition_errors() {
        let text = with_color_override(true, || {
            render_colored_validation_error(DayEntryValidationIssue::InvalidTransition {
                entry_id: EntryId::from(42),
                attempted: CommandLabel::StartWithKind(ActivityKind::Vacation),
                reason: TransitionReason::AlreadyActive(ActivityKind::Work {
                    context: crate::domain::WorkContext::Office,
                }),
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
            let entry = Entry::new(
                1.into(),
                1.into(),
                ts,
                Command::Log { title: "status".into(), description: None },
            );
            let entries = vec![&entry];

            let err = anyhow::Error::from(EntryValidationIndex::try_new(&entries).unwrap_err());
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
            let entry = Entry::new(
                1.into(),
                1.into(),
                ts,
                Command::Log { title: "status".into(), description: None },
            );
            let entries = vec![&entry];

            let err = anyhow::Error::from(EntryValidationIndex::try_new(&entries).unwrap_err())
                .context("outer");

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
