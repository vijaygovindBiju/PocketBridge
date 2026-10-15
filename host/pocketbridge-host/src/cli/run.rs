use anyhow::{Context, Result};
use pocketbridge_core::{
    profiler::build_worker_profile_from_props, read_envelope, write_envelope,
    AdbTransport, AgentRole, ChunkTransformParams, CostModel, DataAnalyticsParams,
    DeviceProfile, Envelope, HeartbeatPayload, HelloPayload, ImageBatchParams,
    MessageKind, RemoteJobClient, Scheduler, SchedulerConfig, AdapterRegistry,
    CoProcessor,
};

#[allow(clippy::too_many_arguments)]
pub async fn execute(
    adapter_name: &str,
    records: u32,
    rounds: u32,
    seed: u64,
    operation: &str,
    width: u32,
    height: u32,
    local_only: bool,
    remote_only: bool,
    hybrid: bool,
    host_ratio: Option<f64>,
    host_load: f64,
    margin: f64,
    port: u16,
) -> Result<()> {
    let registry = AdapterRegistry::with_all_standard();
    let normalized = adapter_name.to_lowercase();

    if matches!(
        normalized.as_str(),
        "llm_pipeline" | "llm" | "llama" | "model" | "llm-llama" | "llm_pipeline.v1"
    ) {
        anyhow::bail!(
            "PocketBridge: Fake simulated LLM execution has been removed from production paths.\n\
             PocketBridge provides real GGUF binary inspection and distributed layer partitioning:\n\
             • Inspect real weights: pocketbridge model inspect gemma2:27b\n\
             • Plan dual-device RAM: pocketbridge model plan gemma2:27b"
        );
    }

    if matches!(
        normalized.as_str(),
        "ai_whisper" | "whisper" | "ai-whisper" | "ai_whisper.v1"
            | "browser_cdp" | "browser" | "browser-cdp" | "browser_cdp.v1"
            | "media_ffmpeg" | "ffmpeg" | "media-ffmpeg" | "media_ffmpeg.v1"
            | "phone_cluster" | "cluster" | "phone-cluster" | "phone_cluster.v1"
    ) {
        anyhow::bail!(
            "Workload adapter '{}' is an unbacked planned roadmap item and does not have a simulated fallback.\n\
             Real supported compute workloads: chunk_transform, image_batch, data_analytics.\n\
             For distributed LLM layer planning, use: pocketbridge model plan gemma2:27b",
            adapter_name
        );
    }

    let adapter_id = match normalized.as_str() {
        "chunk_transform" | "chunk" | "chunk_transform.v1" => "chunk_transform.v1",
        "image_batch" | "image" | "image-batch" | "image_batch.v1" => "image_batch.v1",
        "data_analytics" | "analytics" | "data-analytics" | "data_analytics.v1" => "data_analytics.v1",
        other => {
            if registry.get(other).is_some() {
                other
            } else {
                anyhow::bail!(
                    "Unknown workload adapter '{}'. Available: chunk_transform, image_batch, data_analytics.\n\
                     For neural models, use: pocketbridge model inspect/plan",
                    adapter_name
                );
            }
        }
    };

    let adapter = match registry.get(adapter_id) {
        Some(a) => a,
        None => {
            anyhow::bail!(
                "Workload adapter '{}' is not registered in the core engine.",
                adapter_id
            );
        }
    };

    println!("PocketBridge Compute Workload Runner ({adapter_id})");
    println!("==========================================================================");
    println!("Description: {}", adapter.description());

    let (params_json, task_records, task_rounds) = match adapter_id {
        "chunk_transform.v1" => {
            println!("Parameters:  {} records, {} rounds per record, seed 0x{:016X}", records, rounds, seed);
            (
                serde_json::to_value(ChunkTransformParams {
                    record_count: records,
                    rounds,
                    seed,
                })?,
                records,
                rounds,
            )
        }
        "image_batch.v1" => {
            let img_count = if records == 50_000 { 50 } else { records };
            println!("Parameters:  {} images ({}x{}), operation='{}', seed 0x{:016X}", img_count, width, height, operation, seed);
            (
                serde_json::to_value(ImageBatchParams {
                    image_count: img_count,
                    width,
                    height,
                    seed,
                    operation: operation.to_string(),
                })?,
                img_count,
                width.saturating_mul(height) / 100,
            )
        }
        "data_analytics.v1" => {
            let count = if records == 50_000 { 500_000 } else { records };
            println!("Parameters:  {} tabular records, seed 0x{:016X}, filter range: [0, i64::MAX]", count, seed);
            (
                serde_json::to_value(DataAnalyticsParams {
                    record_count: count,
                    seed,
                    filter_min: 0,
                    filter_max: i64::MAX,
                })?,
                count,
                10,
            )
        }
        _ => {
            (serde_json::json!({ "record_count": records, "seed": seed }), records, rounds)
        }
    };

    adapter.validate_params(&params_json)
        .context("Invalid parameters for workload adapter")?;

    let (in_bytes, out_bytes) = adapter.estimate_payload_sizes(&params_json)
        .unwrap_or((task_records as usize * 8, task_records as usize * 8));

    let config = SchedulerConfig {
        speedup_margin: margin,
        host_load_factor: host_load,
        ..Default::default()
    };
    let scheduler = Scheduler::new(config, CostModel::default());

    // 0. Query live worker profile & RTT if worker is reachable
    let transport = AdbTransport::new(port);
    let mut worker_profile: Option<DeviceProfile> = None;
    let mut rtt_ms: Option<f64> = None;

    if !local_only && transport.check_device().await.is_ok() {
        if let Ok(props) = transport.query_getprop().await {
                let mut profile = build_worker_profile_from_props(&props);
                if transport.setup_forwarding().await.is_ok() {
                    if let Ok(mut stream) = transport.connect_tcp().await {
                        let hello = Envelope::hello("pocketbridge-eval", "0.1.0", AgentRole::Host);
                        let _ = write_envelope(&mut stream, &hello).await;
                        if let Ok(reply) = read_envelope(&mut stream).await {
                            if reply.kind == MessageKind::Hello {
                                let t_ping = std::time::Instant::now();
                                let ping_env = Envelope::benchmark_echo(1, 0, pocketbridge_core::protocol::current_unix_ms());
                                let _ = write_envelope(&mut stream, &ping_env).await;
                                if let Ok(ping_reply) = read_envelope(&mut stream).await {
                                    if ping_reply.kind == MessageKind::BenchmarkEchoReply {
                                        rtt_ms = Some(t_ping.elapsed().as_secs_f64() * 1000.0);
                                    }
                                }

                                let hb = Envelope::heartbeat(HeartbeatPayload {
                                    uptime_ms: 0,
                                    cpu_usage_percent: None,
                                    memory_used_bytes: None,
                                    memory_total_bytes: None,
                                    temperature_celsius: None,
                                    battery_percent: None,
                                    battery_charging: None,
                                    thermal_status: None,
                                });
                                let _ = write_envelope(&mut stream, &hb).await;
                                if let Ok(hb_reply) = read_envelope(&mut stream).await {
                                    if hb_reply.kind == MessageKind::Heartbeat {
                                        if let Ok(worker_hb) = serde_json::from_value::<HeartbeatPayload>(hb_reply.payload) {
                                            profile.battery_percent = worker_hb.battery_percent;
                                            profile.battery_charging = worker_hb.battery_charging;
                                            profile.thermal_status = worker_hb.thermal_status;
                                            if let (Some(used), Some(total)) = (worker_hb.memory_used_bytes, worker_hb.memory_total_bytes) {
                                                profile.ram_total_bytes = total;
                                                profile.ram_available_bytes = Some(total.saturating_sub(used));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                worker_profile = Some(profile);
            }
        }

    // Evaluate Scheduler Decision
    let decision = scheduler.evaluate_task(
        task_records,
        task_rounds,
        in_bytes,
        out_bytes,
        worker_profile.as_ref(),
        rtt_ms,
    );

    println!("\n{}", Scheduler::format_decision_card(&decision));

    if hybrid {
        println!("\n>> Preparing Hybrid Co-Processing (Host + Phone in parallel)...");
        let transport = AdbTransport::new(port);
        let device = transport.check_device().await
            .context("ADB device check failed. Is phone connected via USB debugging?")?;
        println!("   Target device: {device}");

        transport.setup_forwarding().await
            .context("Failed to setup ADB port forwarding")?;

        println!("   Connecting to worker at 127.0.0.1:{port}...");
        let mut stream = transport.connect_tcp().await
            .context("Failed to connect to worker service")?;

        let hello = Envelope::hello("pocketbridge-hybrid", "0.1.0", AgentRole::Host);
        write_envelope(&mut stream, &hello).await?;
        let reply = read_envelope(&mut stream).await?;
        if reply.kind != MessageKind::Hello {
            anyhow::bail!("Expected HELLO response, got {:?}", reply.kind);
        }
        let worker_hello: HelloPayload = serde_json::from_value(reply.payload)?;
        println!("   Handshook with worker: {} (v{})\n", worker_hello.agent_name, worker_hello.agent_version);

        println!(">> Calibrating workload partition ratio based on device capability profiles...");
        let effective_host_ratio = match host_ratio {
            Some(explicit) => {
                println!("   User manual override: {:.1}% Host / {:.1}% Phone", explicit * 100.0, (1.0 - explicit) * 100.0);
                explicit
            }
            None => {
                let host_prof = pocketbridge_core::profiler::collect_host_profile().await
                    .unwrap_or_else(|_| DeviceProfile {
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

                if let Some(w_prof) = &worker_profile {
                    let (cfg, rationale) = pocketbridge_core::PartitionConfig::from_device_profiles(&host_prof, w_prof, host_load);
                    println!("   [DYNAMIC ALLOCATION] {rationale}");
                    cfg.host_ratio
                } else {
                    println!("   Fallback default partition: 50.0% Host / 50.0% Phone");
                    0.5
                }
            }
        };

        println!(">> Launching concurrent hybrid execution across Laptop and Phone...");
        let co_res = CoProcessor::execute_hybrid(
            &mut stream,
            adapter_id,
            &params_json,
            effective_host_ratio,
            |fraction, msg| {
                print!("\r   Progress: [{:>3.0}%] {}", fraction * 100.0, msg);
                use std::io::Write;
                let _ = std::io::stdout().flush();
            },
        ).await.context("Failed executing hybrid co-processing job")?;

        println!("\n\n==========================================================================");
        println!("Hybrid Co-Processing Summary (OPT-C)");
        println!("==========================================================================");
        println!("Adapter ID:               {}", co_res.adapter_id);
        println!("Workload Partitioning:    {:.1}% Host / {:.1}% Phone", co_res.host_ratio * 100.0, (1.0 - co_res.host_ratio) * 100.0);
        println!("Total Records Processed:  {}", co_res.total_records);
        println!("Host Records Processed:   {}", co_res.host_records);
        println!("Phone Records Processed:  {}", co_res.worker_records);
        println!("Host Compute Time:        {:.2} ms", co_res.host_duration_ms);
        println!("Phone Compute Time:       {:.2} ms", co_res.worker_duration_ms);
        println!("Concurrent Wall-Clock:    {:.2} ms (both devices computing in parallel)", co_res.wall_clock_duration_ms);
        println!("Combined Throughput:      {:.0} records/sec", co_res.throughput_records_sec);
        println!("Combined SHA-256 Digest:  {}", co_res.combined_sha256);
        println!("Host Slice SHA-256:       {}", co_res.host_sha256);
        println!("Phone Slice SHA-256:      {}", co_res.worker_sha256);
        println!("--------------------------------------------------------------------------");
        println!("Projected Speedup:        {:.2}x (analytical extrapolation from host slice)", co_res.effective_speedup);
        println!("Note: Standalone full baseline not executed in this run; speedup is an analytical projection.");
        println!("==========================================================================");
        return Ok(());
    }

    // 1. Local Run
    let local_output = if !remote_only {
        println!("\n>> Running locally on Arch Linux host...");
        let out = adapter.execute_local(&params_json)
            .context("Failed executing adapter locally")?;
        println!("   Local compute duration: {:.2} ms", out.duration_ms);
        println!("   Local output SHA-256:   {}", out.output_sha256);
        Some(out)
    } else {
        None
    };

    if local_only {
        println!("\nCompleted (local-only mode).");
        return Ok(());
    }

    // 2. Remote Run via Worker
    println!("\n>> Preparing remote execution on phone...");
    let transport = AdbTransport::new(port);
    let device = transport.check_device().await
        .context("ADB device check failed. Is phone connected via USB debugging?")?;
    println!("   Target device: {device}");

    transport.setup_forwarding().await
        .context("Failed to setup ADB port forwarding")?;

    println!("   Connecting to worker at 127.0.0.1:{port}...");
    let mut stream = transport.connect_tcp().await
        .context("Failed to connect to worker service")?;

    // Handshake
    let hello = Envelope::hello("pocketbridge-run", "0.1.0", AgentRole::Host);
    write_envelope(&mut stream, &hello).await?;
    let reply = read_envelope(&mut stream).await?;
    if reply.kind != MessageKind::Hello {
        anyhow::bail!("Expected HELLO response, got {:?}", reply.kind);
    }
    let worker_hello: HelloPayload = serde_json::from_value(reply.payload)?;
    println!("   Handshook with worker: {} (v{})\n", worker_hello.agent_name, worker_hello.agent_version);

    // Execute remote job
    println!(">> Submitting and monitoring remote job on phone...");
    let result = RemoteJobClient::execute_generic_adapter_job(
        &mut stream,
        adapter_id,
        &params_json,
        local_output.as_ref().map(|o| o.output_sha256.as_str()),
        local_output.as_ref().map(|o| o.duration_ms),
        |fraction, msg| {
            print!("\r   Progress: [{:>3.0}%] {}", fraction * 100.0, msg);
            use std::io::Write;
            let _ = std::io::stdout().flush();
        },
    ).await.context("Failed executing remote job")?;

    println!("\n\n==========================================================================");
    println!("Job Execution Summary");
    println!("==========================================================================");
    println!("Job ID:               {}", result.job_id);
    println!("Adapter ID:           {}", result.adapter_id);
    println!("Final Status:         {}", result.state);
    println!("Records Processed:    {}", result.records_processed);

    if let Some(local_ms) = result.local_duration_ms {
        println!("Local Compute Time:   {:.2} ms", local_ms);
        println!("Local SHA-256:        {}", result.local_sha256.as_deref().unwrap_or("-"));
    }

    if let Some(remote_comp) = result.remote_compute_ms {
        println!("Phone Compute Time:   {:.2} ms", remote_comp);
        if result.adapter_id == "llm_pipeline.v1" && result.records_processed > 0 {
            let iter_sec = (result.records_processed as f64) / (remote_comp / 1000.0);
            let ms_iter = remote_comp / (result.records_processed as f64);
            println!("Phone Memory Stress Rate: {:.2} synthetic_iterations/sec ({:.2} ms/iter)", iter_sec, ms_iter);
            if let Some(bytes) = result.output_size_bytes {
                println!("Phone Direct RAM Dirtied: {:.2} MB allocated in off-heap buffers", bytes as f64 / (1024.0 * 1024.0));
            }
        }
    }
    if let Some(up) = result.remote_upload_ms {
        println!("Network Upload Time:  {:.2} ms", up);
    }
    if let Some(dl) = result.remote_download_ms {
        println!("Network Download Time:{:.2} ms", dl);
    }
    if let Some(total_rem) = result.remote_total_ms {
        println!("Total Remote Elapsed: {:.2} ms (including network transfer & startup)", total_rem);
    }
    if let Some(sha) = &result.remote_sha256 {
        println!("Phone SHA-256:        {}", sha);
    }

    println!("--------------------------------------------------------------------------");
    if result.outputs_match {
        println!("Result Equivalence:   VERIFIED (Both outputs match bit-for-bit!)");
    } else {
        eprintln!("Result Equivalence:   MISMATCH!");
    }

    if let Some(speedup) = result.speedup {
        if speedup > 1.0 {
            println!("End-to-End Speedup:   {:.2}x FASTER remotely (Speedup > 1.0)", speedup);
        } else {
            println!("End-to-End Speedup:   {:.2}x (Local was faster due to transfer/startup overhead)", speedup);
        }
    }
    println!("==========================================================================");

    Ok(())
}
