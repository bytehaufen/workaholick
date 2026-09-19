use crate::{
    App, JsonlFileStore,
    app::AppError,
    cli::{self, CliCommand, ConfigCommand, DispatchError, ShellEditor},
    config::{self, ConfigError},
    store::StoreError,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum RunError {
    #[error(transparent)]
    App(#[from] AppError),

    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Store(#[from] StoreError),

    #[error(transparent)]
    Dispatch(#[from] DispatchError),
}

pub(crate) fn run_app() -> Result<(), RunError> {
    let cli = cli::parse();

    let editor = ShellEditor::real();

    if let CliCommand::Completions { shell } = &cli.command {
        cli::print_completions(*shell);
        return Ok(());
    }

    if let CliCommand::Config { command } = &cli.command {
        let config_override = cli.config.as_deref();

        match command.as_ref().unwrap_or(&ConfigCommand::Show) {
            ConfigCommand::Show => {
                if let Some(path) = config_override {
                    println!("{}", config::show_from(path)?);
                } else {
                    println!("{}", config::show()?);
                }
                return Ok(());
            }
            ConfigCommand::Edit => {
                if let Some(path) = config_override {
                    config::edit_at(path, &editor)?;
                } else {
                    config::edit(&editor)?;
                }
                return Ok(());
            }
            ConfigCommand::Init => {
                if let Some(path) = config_override {
                    let _ = config::init_at(path)?;
                } else {
                    let _ = config::init()?;
                }
                return Ok(());
            }
        }
    }

    let cfg = if let Some(path) = cli.config.as_deref() {
        config::load_from(path)?
    } else {
        config::load()?
    };
    let formatter = cfg.cli.formatter;
    let store = JsonlFileStore::new(&cfg.store_path)?;

    let mut app = App::new(cfg, store)?;

    Ok(cli::run(&mut app, cli, &editor, formatter)?)
}
