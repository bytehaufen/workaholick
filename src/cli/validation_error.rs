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
    err.chain().find_map(|cause| cause.downcast_ref::<DayEntryValidationError>())
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
    use crate::domain::{ActivityKind, CommandLabel, EntryId};
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
}
