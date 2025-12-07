use clap::Parser;

mod core;
mod frontend;

use crate::core::{Cli, Commands};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Some(ref config_path) = cli.config {
        println!("Value for config: {}", config_path.display());
    }

    match &cli.command {
        Commands::Web => {
            todo!("Make a web frontend");
        }
        _ => frontend::cli::handle(&cli),
    }

    Ok(())
}
