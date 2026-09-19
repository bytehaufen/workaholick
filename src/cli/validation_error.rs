use std::error::Error;

use colored::Colorize;

use crate::{
    app::AppError, bootstrap::RunError, cli::DispatchError, domain::TimelineValidationError,
};

pub(crate) fn render_error(err: &RunError) -> String {
    match err {
        RunError::App(AppError::TimelineValidation(error))
        | RunError::Dispatch(DispatchError::App(AppError::TimelineValidation(error))) => {
            render_validation_error(error)
        }
        _ => render_error_chain(err),
    }
}

fn render_validation_error(err: &TimelineValidationError) -> String {
    format!("{}: {err}", "validation error".red().bold())
}

fn render_error_chain(err: &dyn Error) -> String {
    let mut rendered = err.to_string();
    let mut source = err.source();

    while let Some(cause) = source {
        rendered.push_str("\n  caused by: ");
        rendered.push_str(&cause.to_string());
        source = cause.source();
    }

    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::ConfigError,
        domain::{RecordSet, RecordSetError, WorkCommand, validate_records},
        store::{CommitError, StoreError},
        test_support::work_record,
    };
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
            let validation_error = validate_records(&record_set).unwrap_err();
            let err = RunError::from(AppError::from(validation_error));

            render_error(&err)
        });

        assert_eq!(
            text,
            "\u{1b}[1;31mvalidation error\u{1b}[0m: cannot log while idle for 2026-01-12"
        );
    }

    #[test]
    fn render_error_includes_config_read_cause() {
        let err = RunError::Config(ConfigError::Read {
            path: "/myfake.config".into(),
            source: std::io::Error::other("fake error"),
        });

        let rendered = render_error(&err);

        assert_eq!(rendered, "could not read config file /myfake.config\n  caused by: fake error");
    }

    #[test]
    fn render_error_includes_dispatch_store_read_cause() {
        let err = RunError::Dispatch(DispatchError::App(AppError::Store(CommitError::Store(
            StoreError::Read {
                path: "/my/fake/path".into(),
                source: std::io::Error::other("fake error"),
            },
        ))));

        let rendered = render_error(&err);
        assert_eq!(rendered, "could not read records from /my/fake/path\n  caused by: fake error");
    }

    #[test]
    fn render_error_formats_dispatch_validation_error() {
        let rendered = with_color_override(false, || {
            let record = work_record(
                1,
                12,
                10,
                0,
                WorkCommand::Log { title: "status".into(), description: None },
            );

            let records = RecordSet::try_from_records(vec![record]).unwrap();
            let validation_error = validate_records(&records).unwrap_err();
            let err = RunError::Dispatch(DispatchError::App(AppError::TimelineValidation(
                validation_error,
            )));

            render_error(&err)
        });

        assert_eq!(rendered, "validation error: cannot log while idle for 2026-01-12");
    }

    #[test]
    fn render_error_includes_invalid_records_cause_exactly_once() {
        let err = RunError::Store(StoreError::InvalidRecords {
            path: "/my/store.jsonl".into(),
            source: RecordSetError::DuplicateId { id: 67.into() },
        });

        let rendered = render_error(&err);

        assert_eq!(
            rendered,
            "records loaded from /my/store.jsonl are invalid\n  caused by: record with id 67 already exists"
        );
    }
}
