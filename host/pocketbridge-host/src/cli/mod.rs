pub mod bench;
pub mod connect;
pub mod profile;

use clap::Subcommand;
use anyhow::Result;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Launch the interactive TUI dashboard (default)
    Tui,
    /// Run ADB link throughput and latency benchmarks
    Bench {
        /// Number of repetitions per payload size
        #[arg(short, long, default_value_t = 20)]
        runs: u32,
        /// Port for ADB forwarding
        #[arg(short, long, default_value_t = 9876)]
        port: u16,
    },
    /// Query and display hardware profiles of host and worker
    Profile {
        /// Port for ADB forwarding
        #[arg(short, long, default_value_t = 9876)]
        port: u16,
    },
    /// Verify connection with worker using HELLO and HEARTBEAT
    Connect {
        /// Port for ADB forwarding
        #[arg(short, long, default_value_t = 9876)]
        port: u16,
    },
}

pub async fn handle_command(cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Tui => crate::tui::run().await,
        Commands::Bench { runs, port } => bench::run(runs, port).await,
        Commands::Profile { port } => profile::run(port).await,
        Commands::Connect { port } => connect::run(port).await,
    }
}
