use anyhow::{Error, anyhow};
use colored::Colorize;

use crate::domain::{
    CommandLabel, DayEntryValidationError, DayEntryValidationIssue, TransitionReason,
};

pub fn style_error(err: Error) -> Error {
    let Some(validation_error) = find_validation_error(&err) else {
        return err;
    };

    anyhow!(render_colored_validation_error(validation_error.issue()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ActivityKind, Command, CommandLabel, Entry, EntryId, EntryValidationIndex,
    };
    use anyhow::anyhow;
    use chrono::{Local, TimeZone};
    use colored::control;

    #[test]
    fn keeps_structural_errors_plain() {
        let text = render_colored_validation_error(DayEntryValidationIssue::EmptyEntries);

        assert_eq!(text, "Day entries validation failed: entries must not be empty");
    }

    #[test]
    fn colorizes_transition_errors() {
        control::set_override(true);

        let text = render_colored_validation_error(DayEntryValidationIssue::InvalidTransition {
            entry_id: EntryId::from(42),
            attempted: CommandLabel::StartWithKind(ActivityKind::Vacation),
            reason: TransitionReason::AlreadyActive(ActivityKind::Work {
                context: crate::domain::WorkContext::Office,
            }),
        });

        control::unset_override();

        assert!(text.contains("\u{1b}["));
        assert!(text.contains("Entry 42 - cannot"));
    }

    #[test]
    fn style_error_passes_through_non_validation_errors() {
        let err = anyhow!("plain error");
        let styled = style_error(err);

        assert_eq!(styled.to_string(), "plain error");
    }

    #[test]
    fn style_error_reformats_validation_errors() {
        control::set_override(true);

        let ts = Local.with_ymd_and_hms(2026, 1, 12, 10, 0, 0).unwrap();
        let entry = Entry::new(
            1.into(),
            1.into(),
            ts,
            Command::Log { title: "status".into(), description: None },
        );
        let entries = vec![&entry];

        let err = EntryValidationIndex::try_new(&entries).unwrap_err();
        let styled = style_error(err);

        control::unset_override();

        let text = styled.to_string();
        assert!(text.contains("Day entries validation failed"));
        assert!(text.contains("Entry 1 - cannot"));
        assert!(text.contains("\u{1b}["));
    }
}
