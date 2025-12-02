mod cas;
mod common;
mod master;
mod proto;
mod scheduler;
mod worker;

use anyhow::Result;
use clap::Parser;
use master::cli::{run_cli, Cli};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    run_cli(cli).await?;
    Ok(())
}
