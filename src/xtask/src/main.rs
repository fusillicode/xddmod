//! This is an implementation of the [xtask pattern](https://github.com/matklad/cargo-xtask).
//! It contains CLI commands to setup the stage for `xddmod`.

mod import_ddragon_champion;

use clap::Parser;
use import_ddragon_champion::ImportDdragonChampion;

#[derive(Parser)]
#[command(name = "xtask")]
pub enum Command {
    ImportDdragonChampion(ImportDdragonChampion),
}

impl Command {
    async fn run(self) -> anyhow::Result<()> {
        match self {
            Self::ImportDdragonChampion(cmd) => cmd.run().await,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Command::parse().run().await
}
