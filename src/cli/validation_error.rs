use anyhow::Error;
use colored::Colorize;

use crate::domain::TimelineValidationError;

pub fn render_error(err: &Error) -> String {
    if let Some(validation_error) = find_validation_error(err) {
        return render_validation_error(validation_error);
    };

    format!("{err:?}")
}

fn find_validation_error(err: &Error) -> Option<&TimelineValidationError> {
    err.downcast_ref::<TimelineValidationError>()
        .or_else(|| err.chain().find_map(|cause| cause.downcast_ref::<TimelineValidationError>()))
}

fn render_validation_error(err: &TimelineValidationError) -> String {
    format!("{}: {err}", "validation error".red().bold())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{RecordSet, WorkCommand, validate_records},
        test_support::work_record,
    };
    use anyhow::{Context, anyhow};
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
    fn render_error_passes_through_non_validation_errors() {
        let err = anyhow!("plain error");
        let rendered = render_error(&err);

        assert_eq!(rendered, format!("{err:?}"));
    }

    #[test]
    fn render_error_reformats_validation_errors() {
        let text = with_color_override(true, || {
            let record = work_record(
                1,
                12,
                10,
                0,
                WorkCommand::Log { title: "status".into(), description: None },
            );

            let record_set = RecordSet::try_from_records(vec![record]).unwrap();
            let err = validate_records(&record_set).unwrap_err();

            render_error(&err)
        });

        assert_eq!(
            text,
            "\u{1b}[1;31mvalidation error\u{1b}[0m: cannot log while idle for 2026-01-12"
        );
    }

    #[test]
    fn render_error_finds_validation_errors_inside_context() {
        let rendered = with_color_override(false, || {
            let record = work_record(
                1,
                12,
                10,
                0,
                WorkCommand::Log { title: "status".into(), description: None },
            );

            let err = validate_records(&RecordSet::try_from_records(vec![record]).unwrap())
                .context("outer")
                .unwrap_err();

            assert!(find_validation_error(&err).is_some());
            render_error(&err)
        });

        assert_eq!(rendered, "validation error: cannot log while idle for 2026-01-12");
    }
}
