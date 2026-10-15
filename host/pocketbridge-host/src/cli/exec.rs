use std::process::Stdio;
use anyhow::{Context, Result};
use pocketbridge_core::{
    AdbTransport, PartitionConfig, PartitionStrategy,
};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub async fn run(
    cmd: &[String],
    strategy_str: &str,
    manual_ratio: Option<f64>,
    input_file: Option<&str>,
    port: u16,
) -> Result<()> {
    if cmd.is_empty() {
        anyhow::bail!("No command specified. Usage: pocketbridge exec -- [COMMAND] [ARGS...]");
    }

    let program = &cmd[0];
    let args = &cmd[1..];

    let strategy = match strategy_str.to_lowercase().as_str() {
        "line" | "lines" | "text" => PartitionStrategy::LineRecord,
        "byte" | "bytes" | "stream" => PartitionStrategy::ByteChunk,
        "item" | "items" | "files" => PartitionStrategy::ItemList,
        _ => PartitionStrategy::LineRecord,
    };

    println!("PocketBridge Universal Process Partition Proxy (v1.1.0)");
    println!("==========================================================================");
    println!("Target Command:       {} {}", program, args.join(" "));
    println!("Partition Strategy:   {:?}", strategy);

    // 1. Check phone connectivity and live profile
    let transport = AdbTransport::new(port);
    let mut phone_connected = false;
    let mut calculated_ratio = manual_ratio.unwrap_or(0.5);

    if let Ok(dev) = transport.check_device().await {
        println!("Connected Phone:      {}", dev);
        if let Ok(props) = transport.query_getprop().await {
            let worker_prof = pocketbridge_core::profiler::build_worker_profile_from_props(&props);
            let host_prof = pocketbridge_core::collect_host_profile().await
                .unwrap_or_else(|_| pocketbridge_core::DeviceProfile {
                    role: "host".into(),
                    hostname: None,
                    os: "Linux".into(),
                    os_version: "".into(),
                    architecture: "x86_64".into(),
                    cpu_model: "Intel Host CPU".into(),
                    cpu_cores: 8,
                    ram_total_bytes: 16 * 1024 * 1024 * 1024,
                    ram_available_bytes: None,
                    gpu: None,
                    storage_total_bytes: None,
                    storage_available_bytes: None,
                    android_api_level: None,
                    device_model: None,
                    soc: None,
                    battery_percent: None,
                    battery_charging: None,
                    thermal_status: None,
                });

            if manual_ratio.is_none() {
                let (cfg, rationale) = PartitionConfig::from_device_profiles(&host_prof, &worker_prof, 1.0);
                println!("Auto Allocation:      {}", rationale);
                calculated_ratio = cfg.host_ratio;
            } else {
                println!("Manual Ratio:         {:.1}% Host / {:.1}% Phone", calculated_ratio * 100.0, (1.0 - calculated_ratio) * 100.0);
            }
            phone_connected = true;
        }
    } else {
        println!("Connected Phone:      None (Executing 100% locally on Laptop)");
        calculated_ratio = 1.0;
    }
    println!("--------------------------------------------------------------------------");

    // 2. Prepare Input Data to slice (from file, stdin, or arguments)
    let raw_input = if let Some(path) = input_file {
        tokio::fs::read_to_string(path).await
            .context(format!("Failed reading input file {path}"))?
    } else {
        String::new()
    };

    let t_start = std::time::Instant::now();

    if !phone_connected || calculated_ratio >= 0.99 || raw_input.is_empty() {
        // Run standalone locally
        println!(">> Executing process on local host...");
        let mut child = Command::new(program)
            .args(args)
            .stdin(if !raw_input.is_empty() { Stdio::piped() } else { Stdio::inherit() })
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .context(format!("Failed executing '{program}'"))?;

        if !raw_input.is_empty() {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(raw_input.as_bytes()).await?;
            }
        }

        let status = child.wait().await?;
        println!("\n--------------------------------------------------------------------------");
        println!("Local Wall-Clock Time: {:.2} ms (exit code: {:?})", t_start.elapsed().as_secs_f64() * 1000.0, status.code());
        println!("==========================================================================");
        return Ok(());
    }

    // 3. Remote Execution Safety Gate
    // Android worker cannot execute arbitrary host Linux ELF binaries.
    anyhow::bail!(
        "Remote process execution for arbitrary host binary '{}' is currently Scaffolded.\n\
         The Android worker cannot execute host Linux ELF binaries directly without an installed adapter module.\n\
         To run distributed workloads, use an adapter with 'pocketbridge run -a <adapter>' (e.g. chunk_transform, image_batch, data_analytics).",
        program
    );
}
