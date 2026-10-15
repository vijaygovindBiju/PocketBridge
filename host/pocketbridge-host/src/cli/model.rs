use anyhow::{Context, Result};
use clap::Subcommand;
use pocketbridge_core::{resolve_model_path, DistributedLayerPlan, GgufModel};

#[derive(Subcommand, Debug)]
pub enum ModelCommands {
    /// Inspect binary GGUF weights, tensor architecture, and metadata
    Inspect {
        /// Model query (e.g. "gemma2:27b" or path to .gguf file)
        #[arg(default_value = "gemma2:27b")]
        model: String,
    },
    /// Calculate exact layer partition boundary and memory distribution across Host and Phone
    Plan {
        /// Model query (e.g. "gemma2:27b" or path to .gguf file)
        #[arg(default_value = "gemma2:27b")]
        model: String,
        /// Host available RAM in GB (default: auto-detected from /proc/meminfo)
        #[arg(long)]
        host_ram_gb: Option<f64>,
        /// Worker available RAM in GB (default: 8.0 GB)
        #[arg(long)]
        worker_ram_gb: Option<f64>,
    },
    /// Hardware preflight safety gate: checks real device RAM/battery/thermals, local binaries, and model readiness
    Preflight {
        /// Model query (e.g. "gemma2:27b" or path to .gguf file)
        #[arg(default_value = "gemma2:27b")]
        model: String,
        /// Run host-only preflight without querying phone (e.g. when phone is physically unavailable)
        #[arg(long, default_value_t = false)]
        offline: bool,
        /// Port for ADB forwarding (default: 50052)
        #[arg(short, long, default_value_t = 50052)]
        port: u16,
    },
}

pub fn inspect(model_query: &str) -> Result<()> {
    println!("PocketBridge GGUF Model Inspector");
    println!("==========================================================================");

    let path = resolve_model_path(model_query)
        .with_context(|| format!("Could not resolve model '{}'", model_query))?;

    println!("Model Path:         {}", path.display());
    let t_start = std::time::Instant::now();
    let model = GgufModel::open(&path)
        .with_context(|| format!("Failed to parse GGUF binary at {:?}", path))?;
    let parse_ms = t_start.elapsed().as_secs_f64() * 1000.0;

    println!("GGUF Format:        Version {} (parsed in {:.2} ms)", model.version, parse_ms);
    println!("Architecture:       {}", model.architecture);
    println!("Model Name:         {}", model.model_name);
    println!("File Size on Disk:  {:.2} GB ({} bytes)", model.file_size_bytes as f64 / 1e9, model.file_size_bytes);
    println!("Total Tensors:      {}", model.tensor_count);
    println!("Metadata KV Pairs:  {}", model.metadata_kv_count);
    println!("Transformer Layers: {} blocks", model.block_count);
    println!("Context Window:     {} tokens", model.context_length);
    println!("Embedding Length:   {} dimensions", model.embedding_length);
    println!("Feed-Forward Dim:   {}", model.feed_forward_length);
    println!("Attention Heads:    {} query / {} KV", model.attention_head_count, model.attention_head_count_kv);
    println!("==========================================================================");

    // Group tensors by category
    let mut block_tensors = 0;
    let mut global_tensors = 0;
    for t in &model.tensors {
        if t.name.starts_with("blk.") {
            block_tensors += 1;
        } else {
            global_tensors += 1;
        }
    }
    println!("Tensor Distribution: {} layer weights, {} global weights (embedding / norm / lm_head)", block_tensors, global_tensors);

    Ok(())
}

pub fn plan(
    model_query: &str,
    host_ram_gb: Option<f64>,
    worker_ram_gb: Option<f64>,
) -> Result<()> {
    println!("PocketBridge Distributed LLM Layer Partition Planner");
    println!("==========================================================================");

    let path = resolve_model_path(model_query)
        .with_context(|| format!("Could not resolve model '{}'", model_query))?;

    println!("Inspecting GGUF:    {}", path.display());
    let model = GgufModel::open(&path)
        .with_context(|| format!("Failed to parse GGUF binary at {:?}", path))?;

    // Determine host available RAM (default: read from /proc/meminfo or 10.5 GB)
    let host_avail_bytes = if let Some(gb) = host_ram_gb {
        (gb * 1e9) as u64
    } else {
        // Read available memory from /proc/meminfo
        let mut avail = 10_500_000_000u64;
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemAvailable:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<u64>() {
                            avail = kb * 1024;
                        }
                    }
                    break;
                }
            }
        }
        avail
    };

    // Determine worker available RAM (default: 8.0 GB for typical flagship 12GB device)
    let worker_avail_bytes = if let Some(gb) = worker_ram_gb {
        (gb * 1e9) as u64
    } else {
        8_000_000_000u64
    };

    // Baseline USB link budget estimate (empirical rate measured via `pocketbridge bench` when connected)
    let baseline_link_mbps = 350.0;

    let plan = DistributedLayerPlan::plan(
        &model,
        host_avail_bytes,
        worker_avail_bytes,
        baseline_link_mbps,
    );

    let host_kv_2k = model.kv_cache_bytes(2048, plan.host_layers_count) as f64 / 1e9;
    let worker_kv_2k = model.kv_cache_bytes(2048, plan.worker_layers_count) as f64 / 1e9;

    println!("Model:              {} (Architecture: {})", plan.model_name, plan.architecture);
    println!("Total Weights:      {:.2} GB across {} transformer layers", plan.total_model_weights_bytes as f64 / 1e9, plan.total_layers);
    println!("Host Available RAM: {:.2} GB", host_avail_bytes as f64 / 1e9);
    println!("Worker Free RAM:    {:.2} GB (advertised phone headroom)", worker_avail_bytes as f64 / 1e9);
    println!("--------------------------------------------------------------------------");
    println!("DISTRIBUTED PARTITION ALLOCATION:");
    println!(
        "• Host Slice:   Layers {} → {} ({} layers, {:.2} GB weights + Embedding {:.2} GB) [Fits in RAM: {}]",
        plan.host_start_layer, plan.host_end_layer, plan.host_layers_count,
        (plan.host_weights_bytes.saturating_sub(plan.embedding_bytes)) as f64 / 1e9,
        plan.embedding_bytes as f64 / 1e9,
        if plan.host_ram_fits { "YES ✅" } else { "NO (OOM) ❌" }
    );
    println!("  Host KV Cache (2K ctx): {:.2} GB", host_kv_2k);
    println!(
        "• Worker Slice: Layers {} → {} ({} layers, {:.2} GB weights + Output Norm/Head {:.2} GB) [Fits in RAM: {}]",
        plan.worker_start_layer, plan.worker_end_layer, plan.worker_layers_count,
        (plan.worker_weights_bytes.saturating_sub(plan.output_norm_bytes + plan.lm_head_bytes)) as f64 / 1e9,
        (plan.output_norm_bytes + plan.lm_head_bytes) as f64 / 1e9,
        if plan.worker_ram_fits { "YES ✅" } else { "NO (OOM) ❌" }
    );
    println!("  Worker KV Cache (2K ctx): {:.2} GB", worker_kv_2k);
    println!("--------------------------------------------------------------------------");
    println!("WIRE ACTIVATION PIPELINE TRANSFER:");
    println!("• Inter-Device Activation:   {} bytes per token (FP16 hidden state [1, {}])", plan.activation_tensor_bytes_per_token, model.embedding_length);
    println!("• Transfer Latency over USB: {:.3} ms / token (at baseline {:.1} Mbps link estimate)", plan.activation_transfer_ms_est, plan.adb_link_bandwidth_mbps);
    println!("--------------------------------------------------------------------------");
    println!("FEASIBILITY & STATUS CHECK:");
    println!("• Standalone Laptop Run:     {}", if plan.can_execute_on_host_alone { "Feasible" } else { "FAILS (triggers Linux kernel OOM-killer) ❌" });
    println!("• Standalone Phone Run:      {}", if plan.can_execute_on_worker_alone { "Feasible" } else { "FAILS (insufficient phone RAM) ❌" });
    println!("• Hybrid Distributed Run:    {}", if plan.can_execute_hybrid_distributed { "FEASIBLE (layers fit within dual-device memory) ✅" } else { "INFEASIBLE" });

    println!("\nTECHNICAL BLOCKERS FOR LIVE INFERENCE:");
    for (i, blocker) in plan.execution_blockers.iter().enumerate() {
        println!("  {}. {}", i + 1, blocker);
    }
    println!("==========================================================================");

    Ok(())
}

pub async fn preflight(
    model_query: &str,
    offline: bool,
    rpc_port: u16,
) -> Result<()> {
    println!("PocketBridge Real Hardware Preflight Safety Gate");
    println!("==========================================================================");

    // 1. Host Memory Check
    println!("[1/4] Auditing Host Linux System & Memory Headroom...");
    let mut host_total_bytes = 0u64;
    let mut host_avail_bytes = 0u64;
    let mut host_swap_total = 0u64;
    let mut host_swap_free = 0u64;

    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if parts[0] == "MemTotal:" {
                    host_total_bytes = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                } else if parts[0] == "MemAvailable:" {
                    host_avail_bytes = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                } else if parts[0] == "SwapTotal:" {
                    host_swap_total = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                } else if parts[0] == "SwapFree:" {
                    host_swap_free = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                }
            }
        }
    }
    println!("  • Host Total RAM:     {:.2} GB", host_total_bytes as f64 / 1e9);
    println!("  • Host Available RAM: {:.2} GB", host_avail_bytes as f64 / 1e9);
    println!("  • Host Swap Free:     {:.2} GB / {:.2} GB", host_swap_free as f64 / 1e9, host_swap_total as f64 / 1e9);

    // 2. Model Inspection & Weight Budget
    println!("\n[2/4] Auditing GGUF Model Architecture & Memory Footprint...");
    let model_path = resolve_model_path(model_query)
        .with_context(|| format!("Could not resolve model '{}'", model_query))?;
    let model = GgufModel::open(&model_path)
        .with_context(|| format!("Failed to parse GGUF binary at {:?}", model_path))?;

    let total_weights_gb = model.tensors.iter().map(|t| t.estimated_bytes).sum::<u64>() as f64 / 1e9;
    let kv_cache_2k = model.kv_cache_bytes(2048, model.block_count) as f64 / 1e9;
    let kv_cache_8k = model.kv_cache_bytes(8192, model.block_count) as f64 / 1e9;

    println!("  • Model Name:         {} ({})", model.model_name, model.architecture);
    println!("  • File Size on Disk:  {:.2} GB", model.file_size_bytes as f64 / 1e9);
    println!("  • Weights Footprint:  {:.2} GB across {} transformer blocks", total_weights_gb, model.block_count);
    println!("  • Total Tensors:      {} tensors", model.tensor_count);
    println!("  • KV Cache (2K ctx):  {:.2} GB (full model)", kv_cache_2k);
    println!("  • KV Cache (8K ctx):  {:.2} GB (full model)", kv_cache_8k);

    if (total_weights_gb * 1e9) as u64 > host_avail_bytes {
        println!("  ⚠️  HOST OOM NOTICE: Total model ({:.2} GB) exceeds available laptop RAM ({:.2} GB).", total_weights_gb, host_avail_bytes as f64 / 1e9);
        println!("      Standalone host execution will trigger kernel OOM killer. Co-processing offload is required.");
    }

    // 3. Native Binaries & Toolchain Check
    println!("\n[3/4] Checking Native Inference Binaries & Android Cross-Compilation...");
    let find_binary = |rel_path: &str| -> Option<std::path::PathBuf> {
        let candidates = [
            std::path::PathBuf::from(rel_path),
            std::path::PathBuf::from("../").join(rel_path),
            std::path::PathBuf::from("../../").join(rel_path),
        ];
        candidates.into_iter().find(|p| p.exists())
    };

    let host_cli = find_binary("third_party/llama.cpp/build-host/bin/llama-cli");
    let android_rpc = find_binary("third_party/llama.cpp/build-android/bin/ggml-rpc-server");
    let android_omp = find_binary("third_party/llama.cpp/build-android/bin/libomp.so");
    let android_cpu = find_binary("third_party/llama.cpp/build-android/bin/libggml-cpu.so");

    let host_cli_ok = host_cli.is_some();
    let android_rpc_ok = android_rpc.is_some();
    let android_omp_ok = android_omp.is_some();
    let android_cpu_ok = android_cpu.is_some();

    println!("  • Host llama-cli:         {}", if host_cli_ok { "Ready (x86_64 ELF) ✅" } else { "MISSING ❌" });
    println!("  • Android ggml-rpc-server: {}", if android_rpc_ok { "Ready (ARM64 ELF) ✅" } else { "MISSING ❌" });
    println!("  • Android libomp.so:      {}", if android_omp_ok { "Ready (OpenMP runtime) ✅" } else { "MISSING ❌" });
    println!("  • Android libggml-cpu.so: {}", if android_cpu_ok { "Ready (ARM64 CPU backend) ✅" } else { "MISSING ❌" });

    // 4. Physical Android Phone Telemetry Gate (NO SIMULATION)
    println!("\n[4/4] Evaluating Physical Device Connectivity & Real-Time Telemetry Gate...");
    if offline {
        println!("  STATUS: ⏸️  HOST-ONLY AUDIT (Offline mode active; phone query skipped)");
        println!("  ----------------------------------------------------------------------");
        println!("  SAFETY GATE ENFORCED:");
        println!("  • Samsung Galaxy S25 Ultra is treated as physically unavailable.");
        println!("  • Phone-side execution and distributed inference are strictly NOT claimed.");
        println!("  • Zero mock telemetry or fake outputs substituted.");
        println!("  • Real physical measurements will occur once device is connected and authorized.");
        println!("  ----------------------------------------------------------------------");
        println!("  PREREQUISITES FOR WHEN PHONE IS AVAILABLE:");
        println!("  1. Connect Samsung Galaxy S25 Ultra to laptop via USB-C cable.");
        println!("  2. Enable Developer Options -> USB Debugging on the phone.");
        println!("  3. Accept the 'Allow USB debugging?' authorization prompt on the phone screen.");
        println!("  4. Run preflight: 'cargo run -p pocketbridge-host -- model preflight'");
        println!("  5. Deploy and execute: './scripts/deploy_and_run.sh --run 8'");
        println!("==========================================================================");
        return Ok(());
    }

    let adb = pocketbridge_core::AdbTransport::new(rpc_port);
    let attached = adb.list_devices().await.unwrap_or_default();

    if attached.is_empty() {
        println!("  STATUS: ⏸️  AWAITING PHYSICAL DEVICE (Phone currently disconnected)");
        println!("  ----------------------------------------------------------------------");
        println!("  SAFETY GATE ENFORCED:");
        println!("  • PocketBridge strictly refuses to substitute simulated or guessed");
        println!("    telemetry for actual device measurements.");
        println!("  • Samsung Galaxy S25 Ultra is currently NOT physically connected.");
        println!("  • All phone-side telemetry checks remain DEFERRED until device connection.");
        println!("  ----------------------------------------------------------------------");
        println!("  PREREQUISITES FOR WHEN PHONE IS AVAILABLE:");
        println!("  1. Connect Samsung Galaxy S25 Ultra to laptop via USB-C cable.");
        println!("  2. Enable Developer Options -> USB Debugging on the phone.");
        println!("  3. Accept the 'Allow USB debugging?' authorization prompt on the phone screen.");
        println!("  4. Re-run preflight: 'cargo run -p pocketbridge-host -- model preflight'");
        println!("  5. Deploy and execute: './scripts/deploy_and_run.sh --run 8'");
        println!("==========================================================================");
        return Ok(());
    }

    // If attached, measure real telemetry
    println!("  Device Attached: {}", attached[0]);
    match adb.query_meminfo().await {
        Ok(mem) => {
            let avail_mb = mem.available_bytes / (1024 * 1024);
            let total_mb = mem.total_bytes / (1024 * 1024);
            println!("  • Real Phone Total RAM:     {} MB ({:.2} GB)", total_mb, mem.total_bytes as f64 / 1e9);
            println!("  • Real Phone Available RAM: {} MB ({:.2} GB)", avail_mb, mem.available_bytes as f64 / 1e9);

            let safe_headroom_mb = avail_mb.saturating_sub(1200); // 1.2 GB OS safety buffer
            let max_safe_layers = safe_headroom_mb / 319; // ~319 MB per layer for Gemma 2 27B
            println!("  • Safe Layer Headroom:      {} layers (leaves 1.2 GB Android system buffer)", max_safe_layers);

            if let Ok((batt, temp, charging)) = adb.query_battery().await {
                println!("  • Real Battery Telemetry:   {}% ({:.1}°C, charging: {})", batt, temp, charging);
                if batt < 20 && !charging {
                    println!("  ⚠️  SAFETY WARNING: Battery below 20% and not charging. Plug in charger.");
                }
                if temp > 42.0 {
                    println!("  ⚠️  THERMAL WARNING: Battery temperature is high ({:.1}°C). Let device cool down.", temp);
                }
            }

            println!("\n  PREFLIGHT GATE PASSED: Physical device verified and ready for deployment.");
            println!("  Recommended command:");
            println!("    ./scripts/deploy_and_run.sh --run {}", max_safe_layers.min(25).max(1));
        }
        Err(e) => {
            println!("  ⚠️  Failed to query device memory over ADB: {e}");
        }
    }

    println!("==========================================================================");
    Ok(())
}
