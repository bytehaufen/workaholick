use std::process::ExitCode;
use workaholick::{cli, run_app};

fn main() -> ExitCode {
    match run_app() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", cli::render_error(&err));
            ExitCode::FAILURE
        }
    }
}
