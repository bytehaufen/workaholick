use anyhow::Result;
use workaholick::{
    App, JsonlFileStore,
    cli::{self, CliCommand, ConfigCommand, ShellEditor},
    config,
};

fn main() -> Result<()> {
    let cli = cli::parse();

    let editor = ShellEditor::real();

    if let CliCommand::Config { command } = cli.command {
        match command.unwrap_or(ConfigCommand::Show) {
            ConfigCommand::Show => {
                println!("{}", config::show()?);
                return Ok(());
            }
            ConfigCommand::Edit => {
                config::edit(&editor)?;
                return Ok(());
            }
            ConfigCommand::Init => {
                config::init()?;
                return Ok(());
            }
        }
    }

    let cfg = config::load()?;
    let store = JsonlFileStore::new(&cfg.store_path)?;

    let mut app = App::new(cfg, store, editor)?;

    cli::run(&mut app, cli)
}
