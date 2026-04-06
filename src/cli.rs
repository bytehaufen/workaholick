mod args;
mod dispatch;
mod edit_format;
mod format;
mod shell_editor;
mod validation_error;

use clap::CommandFactory;
use clap_complete::generate;
use std::io;

pub use args::Cli;
pub use args::CliCommand;
pub use args::CompletionShell;
pub use args::ConfigCommand;
pub use args::parse;
pub use dispatch::run;
#[cfg(feature = "markdown")]
pub use format::MarkdownFormatter;
pub use format::{Formatter, PlainFormatter};
pub use shell_editor::ShellEditor;
pub use validation_error::render_error;

pub fn print_completions(shell: CompletionShell) {
    let mut cmd = Cli::command();
    let shell: clap_complete::Shell = shell.into();

    generate(shell, &mut cmd, env!("CARGO_PKG_NAME"), &mut io::stdout());
}
