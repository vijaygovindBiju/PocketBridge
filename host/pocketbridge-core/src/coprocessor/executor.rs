use std::time::Instant;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::net::TcpStream;
use tracing::info;

use crate::adapter::AdapterRegistry;
use crate::error::PocketBridgeError;
use crate::job::{JobExecutionResult, JobState, RemoteJobClient};
use super::partition::partition_workload;

/// Detailed outcome of a concurrent co-processing execution spanning Host and Worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoProcessingResult {
    pub adapter_id: String,
    pub host_ratio: f64,
    pub total_records: u64,
    pub host_records: u64,
    pub worker_records: u64,
    pub host_duration_ms: f64,
    pub worker_duration_ms: f64,
    pub wall_clock_duration_ms: f64,
    pub combined_sha256: String,
    pub host_sha256: String,
    pub worker_sha256: String,
    pub effective_speedup: f64,
    pub throughput_records_sec: f64,
    pub summary: String,
}

pub struct CoProcessor;

impl CoProcessor {
    /// Execute a workload collaboratively across both local Host CPU cores and remote Phone Worker.
    pub async fn execute_hybrid<F>(
        stream: &mut TcpStream,
        adapter_id: &str,
        params: &serde_json::Value,
        host_ratio: f64,
        mut progress: F,
    ) -> Result<CoProcessingResult, PocketBridgeError>
    where
        F: FnMut(f32, &str),
    {
        let registry = AdapterRegistry::default();
        let adapter = registry.get(adapter_id).ok_or_else(|| {
            PocketBridgeError::AdapterError(format!("Adapter not found: {adapter_id}"))
        })?;

        let (host_params, worker_params) = partition_workload(adapter_id, params, host_ratio)?;

        info!(
            "Initiating hybrid co-processing on {adapter_id} (Host ratio: {:.1}%, Phone ratio: {:.1}%)",
            host_ratio * 100.0,
            (1.0 - host_ratio) * 100.0
        );

        progress(0.05, "Partitioned workload. Launching concurrent host & phone executions...");

        let t_wall_start = Instant::now();

        // 1. Launch local execution on background thread
        let host_adapter = adapter.clone();
        let host_params_clone = host_params.clone();
        let host_future = tokio::task::spawn_blocking(move || {
            host_adapter.execute_local(&host_params_clone)
        });

        // 2. Launch remote execution over ADB stream
        let worker_job_future = RemoteJobClient::execute_generic_adapter_job(
            stream,
            adapter_id,
            &worker_params,
            None,
            None,
            |fraction, msg| {
                tracing::debug!("Worker co-processing progress: [{:.0}%] {}", fraction * 100.0, msg);
            },
        );

        // Await both concurrently
        let (host_join_res, worker_res) = tokio::join!(host_future, worker_job_future);

        let wall_clock_duration_ms = t_wall_start.elapsed().as_secs_f64() * 1000.0;

        let host_exec_result = host_join_res
            .map_err(|e| PocketBridgeError::CoProcessorError(format!("Host thread panic: {e}")))?
            .map_err(|e| PocketBridgeError::CoProcessorError(format!("Host execution failed: {e}")))?;

        let worker_exec_result: JobExecutionResult = worker_res?;

        if worker_exec_result.state != JobState::Succeeded {
            return Err(PocketBridgeError::CoProcessorError(format!(
                "Worker failed during co-processing: {:?}",
                worker_exec_result.error_message
            )));
        }

        let worker_sha = worker_exec_result.remote_sha256.clone().unwrap_or_default();
        let worker_duration = worker_exec_result.remote_compute_ms.unwrap_or(worker_exec_result.remote_total_ms.unwrap_or(0.0));

        // Generate combined deterministic SHA-256
        let mut hasher = Sha256::new();
        hasher.update(host_exec_result.output_sha256.as_bytes());
        hasher.update(b"::");
        hasher.update(worker_sha.as_bytes());
        let combined_sha256 = format!("{:x}", hasher.finalize());

        let host_records = match adapter_id {
            crate::adapter::ADAPTER_ID => {
                let p: crate::adapter::ChunkTransformParams = serde_json::from_value(host_params)?;
                p.record_count as u64
            }
            crate::adapter::IMAGE_BATCH_ADAPTER_ID => {
                let p: crate::adapter::ImageBatchParams = serde_json::from_value(host_params)?;
                p.image_count as u64
            }
            crate::adapter::DATA_ANALYTICS_ADAPTER_ID => {
                let p: crate::adapter::DataAnalyticsParams = serde_json::from_value(host_params)?;
                p.record_count as u64
            }
            _ => 1,
        };

        let worker_records = worker_exec_result.records_processed;
        let total_records = host_records + worker_records;

        // Effective standalone host time estimate = host_duration / host_ratio
        let host_standalone_estimate = if host_ratio > 0.0 {
            host_exec_result.duration_ms / host_ratio
        } else {
            wall_clock_duration_ms
        };

        let effective_speedup = if wall_clock_duration_ms > 0.0 {
            (host_standalone_estimate / wall_clock_duration_ms).max(0.1)
        } else {
            1.0
        };

        let throughput_records_sec = if wall_clock_duration_ms > 0.0 {
            (total_records as f64) / (wall_clock_duration_ms / 1000.0)
        } else {
            0.0
        };

        progress(1.0, "Hybrid co-processing completed and merged successfully");

        Ok(CoProcessingResult {
            adapter_id: adapter_id.to_string(),
            host_ratio,
            total_records,
            host_records,
            worker_records,
            host_duration_ms: host_exec_result.duration_ms,
            worker_duration_ms: worker_duration,
            wall_clock_duration_ms,
            combined_sha256,
            host_sha256: host_exec_result.output_sha256,
            worker_sha256: worker_sha,
            effective_speedup,
            throughput_records_sec,
            summary: format!(
                "Processed {} records across Host ({}) and Phone ({}) in {:.2} ms ({:.2}x speedup, {:.0} rec/s)",
                total_records, host_records, worker_records, wall_clock_duration_ms, effective_speedup, throughput_records_sec
            ),
        })
    }
}
