mod args;
mod dispatch;
mod edit_format;
mod format;
mod shell_editor;
mod validation_error;

use clap::CommandFactory;
use clap_complete::generate;
use std::io;

pub(crate) use args::{Cli, CliCommand, CompletionShell, ConfigCommand, parse};
pub(crate) use dispatch::{DispatchError, run};
pub(crate) use validation_error::render_error;

pub(crate) use shell_editor::ShellEditor;

pub fn print_completions(shell: CompletionShell) {
    let mut cmd = Cli::command();
    let shell: clap_complete::Shell = shell.into();

    generate(shell, &mut cmd, env!("CARGO_PKG_NAME"), &mut io::stdout());
}
