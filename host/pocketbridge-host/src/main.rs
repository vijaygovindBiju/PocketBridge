use clap::Parser;
use anyhow::Result;

mod cli;
mod tui;

#[derive(Parser, Debug)]
#[command(
    name = "pocketbridge",
    version = "0.1.0",
    about = "PocketBridge — Android compute offloader & link feasibility toolkit"
)]
struct Args {
    #[command(subcommand)]
    command: Option<cli::Commands>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Some(cmd) => cli::handle_command(cmd).await,
        None => tui::run().await, // Default to interactive TUI
    }
}
