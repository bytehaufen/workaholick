use crate::core::cli::{Cli, Commands};

pub fn handle(cli: &Cli) {
    println!("i handle it");

    match &cli.command {
        Commands::In { place, description } => {
            println!("In: @{:#?} {:#?}", place, description);
        }
        _ => todo!(),
    }
}
