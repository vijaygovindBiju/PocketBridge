use anyhow::{Context, Result};
use pocketbridge_core::{
    format_summary_table, read_envelope, run_benchmark_suite, save_benchmark_report,
    write_envelope, AdbTransport, AgentRole, BenchmarkConfig, Envelope, HelloPayload, MessageKind,
};

pub async fn run(runs: u32, port: u16) -> Result<()> {
    println!("PocketBridge Link Benchmark");
    println!("--------------------------------");
    println!("Runs per payload size: {runs}");
    println!("Port: {port}\n");

    let transport = AdbTransport::new(port);

    // 1. Check device
    let device = transport.check_device().await
        .context("ADB device check failed")?;
    println!("Target device: {device}");

    // 2. Setup forwarding
    transport.setup_forwarding().await
        .context("Failed to set up ADB port forwarding")?;

    // 3. Connect TCP
    println!("Connecting to worker at 127.0.0.1:{port}...");
    let mut stream = transport.connect_tcp().await
        .context("Failed to connect to worker")?;

    // 4. Handshake
    let hello = Envelope::hello("pocketbridge-bench", "0.1.0", AgentRole::Host);
    write_envelope(&mut stream, &hello).await?;
    let reply = read_envelope(&mut stream).await?;
    if reply.kind != MessageKind::Hello {
        anyhow::bail!("Expected HELLO response, got {:?}", reply.kind);
    }
    let worker_hello: HelloPayload = serde_json::from_value(reply.payload)?;
    println!("Connected to worker: {} (v{})\n", worker_hello.agent_name, worker_hello.agent_version);

    // 5. Run benchmarks
    let config = BenchmarkConfig {
        payload_sizes: vec![1_000_000, 10_000_000, 100_000_000],
        runs_per_size: runs,
        include_latency_pings: true,
        latency_ping_sizes: vec![64, 1024],
        priority: Some(1),
    };

    println!("Starting benchmark suite (latency pings + 1MB, 10MB, 100MB throughput)...");
    let mut last_stage = String::new();

    let report = run_benchmark_suite(&mut stream, &config, |completed, total, stage| {
        if stage != last_stage {
            println!("\n>> Running: {stage}");
            last_stage = stage.to_string();
        }
        print!("\rProgress: [{}/{}] ({:.0}%)", completed, total, (completed as f64 / total as f64) * 100.0);
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }).await.context("Benchmark execution failed")?;

    println!("\n\nBenchmark completed!\n");

    // 6. Display table
    let table = format_summary_table(&report.results);
    println!("{table}");

    // 7. Save report
    let results_dir = "benchmarks/results";
    match save_benchmark_report(&report, results_dir).await {
        Ok(path) => println!("Full benchmark report saved to: {}", path.display()),
        Err(e) => eprintln!("Warning: Failed to save JSON report: {e}"),
    }

    Ok(())
}
