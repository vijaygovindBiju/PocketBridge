pub mod bench;
pub mod connect;
pub mod plan;
pub mod profile;
pub mod run;

use clap::Subcommand;
use anyhow::Result;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Launch the interactive TUI dashboard (default)
    Tui,
    /// Execute a compute workload (chunk_transform, image_batch, data_analytics) locally and/or remotely
    Run {
        /// Workload adapter: chunk_transform (default), image_batch, data_analytics
        #[arg(short = 'a', long, default_value = "chunk_transform")]
        adapter: String,
        /// Number of records or images to process
        #[arg(short, long, default_value_t = 50_000)]
        records: u32,
        /// Number of mathematical rounds per record (for chunk_transform)
        #[arg(short = 'm', long, default_value_t = 20)]
        rounds: u32,
        /// PRNG seed for reproducible data generation
        #[arg(short, long, default_value_t = 0x123456789ABCDEF0)]
        seed: u64,
        /// Image processing operation (for image_batch: grayscale_invert, box_blur_3x3, downscale_half)
        #[arg(long, default_value = "grayscale_invert")]
        operation: String,
        /// Image width (for image_batch)
        #[arg(long, default_value_t = 256)]
        width: u32,
        /// Image height (for image_batch)
        #[arg(long, default_value_t = 256)]
        height: u32,
        /// Execute only on the local Linux host
        #[arg(long, default_value_t = false)]
        local_only: bool,
        /// Execute only on the remote phone worker
        #[arg(long, default_value_t = false)]
        remote_only: bool,
        /// Simulated host load capacity factor (1.0 = idle host, 0.2 = heavy host load)
        #[arg(long, default_value_t = 1.0)]
        host_load: f64,
        /// Required speedup margin to justify offloading (default: 1.15)
        #[arg(long, default_value_t = 1.15)]
        margin: f64,
        /// Port for ADB forwarding
        #[arg(short, long, default_value_t = 9876)]
        port: u16,
    },
    /// Evaluate explainable scheduler recommendations without executing
    Plan {
        /// Number of records to transform
        #[arg(short, long, default_value_t = 500_000)]
        records: u32,
        /// Number of mathematical rounds per record
        #[arg(short = 'm', long, default_value_t = 50)]
        rounds: u32,
        /// Simulated host load capacity factor (1.0 = idle host, 0.2 = heavy host load)
        #[arg(long, default_value_t = 1.0)]
        host_load: f64,
        /// Required speedup margin to justify offloading (default: 1.15)
        #[arg(long, default_value_t = 1.15)]
        margin: f64,
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
        Commands::Run {
            adapter,
            records,
            rounds,
            seed,
            operation,
            width,
            height,
            local_only,
            remote_only,
            host_load,
            margin,
            port,
        } => {
            run::execute(
                &adapter,
                records,
                rounds,
                seed,
                &operation,
                width,
                height,
                local_only,
                remote_only,
                host_load,
                margin,
                port,
            )
            .await
        }
        Commands::Plan { records, rounds, host_load, margin, port } => {
            plan::run(records, rounds, host_load, margin, port).await
        }
        Commands::Bench { runs, port } => bench::run(runs, port).await,
        Commands::Profile { port } => profile::run(port).await,
        Commands::Connect { port } => connect::run(port).await,
    }
}
