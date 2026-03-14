use anyhow::Result;
use workaholick::{
    App, JsonlFileStore,
    cli::{self, CliCommand, ConfigCommand, ShellEditor},
    config,
};

fn main() -> Result<()> {
    let cli = cli::parse();

    let editor = ShellEditor::real();

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
                    config::init_at(path)?;
                } else {
                    config::init()?;
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
    let store = JsonlFileStore::new(&cfg.store_path)?;

    let mut app = App::new(cfg, store)?;

    cli::run(&mut app, cli, &editor)
}
