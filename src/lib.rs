mod app;
mod bootstrap;
pub mod cli;
pub mod config;
mod domain;
mod editor;
mod store;

use std::process::ExitCode;

pub(crate) use crate::app::App;
pub(crate) use crate::app::DayOverview;
pub(crate) use crate::store::JsonlFileStore;

#[cfg(test)]
/// Shared test helpers
mod test_support;

pub fn run() -> ExitCode {
    match bootstrap::run_app() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", cli::render_error(&err));
            ExitCode::FAILURE
        }
    }
}
