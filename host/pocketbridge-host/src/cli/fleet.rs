use std::sync::Arc;
use anyhow::{Context, Result};
use pocketbridge_core::{
    AdapterRegistry, ChunkTransformParams, DataAnalyticsParams, FleetCoordinator,
    FleetManager, FleetWorkQueue, ImageBatchParams, WorkChunk,
};
use tokio::sync::Mutex;

pub async fn run(
    adapter_name: &str,
    records: u32,
    chunks_count: usize,
    rounds: u32,
    seed: u64,
    base_port: u16,
) -> Result<()> {
    let registry = AdapterRegistry::default();
    let normalized = adapter_name.to_lowercase();
    let adapter_id = match normalized.as_str() {
        "chunk_transform" | "chunk" | "chunk_transform.v1" => "chunk_transform.v1",
        "image_batch" | "image" | "image_batch.v1" => "image_batch.v1",
        "data_analytics" | "analytics" | "data_analytics.v1" => "data_analytics.v1",
        other => {
            if registry.get(other).is_some() {
                other
            } else {
                anyhow::bail!("Unknown workload adapter '{}'. Available: chunk_transform, image_batch, data_analytics", adapter_name);
            }
        }
    };

    let adapter = registry.get(adapter_id).context("Adapter not found in registry")?;

    println!("PocketBridge Multi-Worker Fleet Coordinator ({adapter_id})");
    println!("==========================================================================");
    println!("Description: {}", adapter.description());
    println!("Total Work:  {} items divided across {} chunks", records, chunks_count);
    println!("Base Port:   {}", base_port);
    println!("--------------------------------------------------------------------------");

    // 1. Discover ADB workers
    println!("Scanning connected ADB worker fleet...");
    let mut fleet = FleetManager::new(base_port);
    let count = fleet.discover_and_setup().await
        .context("Failed to run ADB fleet discovery")?;

    if count == 0 {
        anyhow::bail!("No online ADB worker devices discovered. Connect phone(s) with USB debugging enabled.");
    }

    println!("Discovered {} worker node(s):", count);
    for node in fleet.active_nodes() {
        println!("  - Worker: {:<20} -> Forwarded Port: {}", node.serial, node.port);
    }
    println!("--------------------------------------------------------------------------");

    // 2. Prepare Work Chunks
    let chunk_size = (records as usize + chunks_count - 1) / chunks_count;
    let queue = Arc::new(Mutex::new(FleetWorkQueue::new()));

    {
        let mut q = queue.lock().await;
        for i in 0..chunks_count {
            let offset = (i * chunk_size) as u64;
            let this_chunk_records = if i == chunks_count - 1 {
                records as usize - (i * chunk_size)
            } else {
                chunk_size
            } as u32;

            let params = match adapter_id {
                "chunk_transform.v1" => serde_json::to_value(ChunkTransformParams {
                    record_count: this_chunk_records,
                    rounds,
                    seed: seed.wrapping_add(offset),
                })?,
                "image_batch.v1" => serde_json::to_value(ImageBatchParams {
                    image_count: this_chunk_records,
                    width: 256,
                    height: 256,
                    seed: seed.wrapping_add(offset),
                    operation: "grayscale_invert".to_string(),
                })?,
                "data_analytics.v1" => serde_json::to_value(DataAnalyticsParams {
                    record_count: this_chunk_records,
                    seed: seed.wrapping_add(offset),
                    filter_min: 0,
                    filter_max: i64::MAX,
                })?,
                _ => serde_json::json!({ "record_count": this_chunk_records, "seed": seed.wrapping_add(offset) }),
            };

            q.push_chunk(WorkChunk {
                chunk_id: i as u32,
                adapter_id: adapter_id.to_string(),
                parameters: params,
                records_count: this_chunk_records as u64,
            });
        }
    }

    println!("Distributing {} chunks across fleet using work-stealing...", chunks_count);

    // 3. Execute Distributed Workload
    let summary = FleetCoordinator::execute_distributed(
        &mut fleet,
        queue,
        |completed, total, msg| {
            println!("  [{:>2}/{:>2}] {}", completed, total, msg);
        },
    )
    .await
    .context("Distributed fleet execution failed")?;

    // 4. Report Fleet Performance
    println!("\n==========================================================================");
    println!("Fleet Execution Summary");
    println!("==========================================================================");
    println!("  Total Chunks:          {}", summary.total_chunks);
    println!("  Total Items Processed: {}", summary.total_records);
    println!("  Fleet Wall-Clock Time: {:.2} ms", summary.wall_clock_duration_ms);
    println!("  Fleet Throughput:      {:.2} items/sec", summary.throughput_records_sec);
    println!("  Combined SHA-256:      {}", summary.combined_sha256);
    println!("\nWorker Chunk Distribution:");
    for (serial, chunks) in &summary.worker_chunk_counts {
        println!("  - {:<24} completed {} chunk(s)", serial, chunks);
    }
    println!("==========================================================================");

    // 5. Clean up forwarding
    fleet.clean_forwarding().await;

    Ok(())
}
