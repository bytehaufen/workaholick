use anyhow::Result;
use clap::Parser;

use crate::{
    core::{
        App,
        cli::{Cli, Commands},
    },
    frontend::web,
};

pub fn run(app: &App) -> Result<()> {
    let cli = Cli::parse();

    if let Some(ref config_path) = cli.config {
        println!("Value for config: {}", config_path.display());
    }

    match &cli.command {
        Commands::Web => web::run(app),

        Commands::In { place, description } => {
            println!("In: @{:#?} {:#?}", place, description);
            Ok(())
        }
        _ => todo!("implement"),
    }
}
