pub mod bench;
pub mod connect;
pub mod profile;
pub mod run;

use clap::Subcommand;
use anyhow::Result;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Launch the interactive TUI dashboard (default)
    Tui,
    /// Execute a compute workload (chunk_transform.v1) locally and/or remotely
    Run {
        /// Number of records to transform
        #[arg(short, long, default_value_t = 50_000)]
        records: u32,
        /// Number of mathematical rounds per record
        #[arg(short = 'm', long, default_value_t = 20)]
        rounds: u32,
        /// PRNG seed for reproducible data generation
        #[arg(short, long, default_value_t = 0x123456789ABCDEF0)]
        seed: u64,
        /// Execute only on the local Linux host
        #[arg(long, default_value_t = false)]
        local_only: bool,
        /// Execute only on the remote phone worker
        #[arg(long, default_value_t = false)]
        remote_only: bool,
        /// Port for ADB forwarding
        #[arg(short, long, default_value_t = 9876)]
        port: u16,
    },
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
        Commands::Run { records, rounds, seed, local_only, remote_only, port } => {
            run::execute(records, rounds, seed, local_only, remote_only, port).await
        }
        Commands::Bench { runs, port } => bench::run(runs, port).await,
        Commands::Profile { port } => profile::run(port).await,
        Commands::Connect { port } => connect::run(port).await,
    }
}
