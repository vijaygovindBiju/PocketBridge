use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Instant;
use super::traits::{AdapterExecutionResult, WorkloadAdapter};
use crate::error::PocketBridgeError;

pub const DATA_ANALYTICS_ADAPTER_ID: &str = "data_analytics.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataAnalyticsParams {
    pub record_count: u32,
    pub seed: u64,
    pub filter_min: i64,
    pub filter_max: i64,
}

impl Default for DataAnalyticsParams {
    fn default() -> Self {
        Self {
            record_count: 500_000,
            seed: 0xDEADBEEF01234567,
            filter_min: 0,
            filter_max: i64::MAX,
        }
    }
}

pub struct DataAnalyticsAdapter;

impl WorkloadAdapter for DataAnalyticsAdapter {
    fn adapter_id(&self) -> &'static str {
        DATA_ANALYTICS_ADAPTER_ID
    }

    fn description(&self) -> &'static str {
        "High-throughput tabular record generation, range filtering, and statistical map-reduce"
    }

    fn validate_params(&self, params: &serde_json::Value) -> Result<(), PocketBridgeError> {
        let parsed: DataAnalyticsParams = serde_json::from_value(params.clone())
            .map_err(|e| PocketBridgeError::AdapterError(format!("Invalid DataAnalyticsParams: {e}")))?;
        if parsed.record_count == 0 {
            return Err(PocketBridgeError::AdapterError("record_count must be positive".into()));
        }
        if parsed.filter_min > parsed.filter_max {
            return Err(PocketBridgeError::AdapterError("filter_min cannot exceed filter_max".into()));
        }
        Ok(())
    }

    fn estimate_payload_sizes(&self, params: &serde_json::Value) -> Result<(usize, usize), PocketBridgeError> {
        let parsed: DataAnalyticsParams = serde_json::from_value(params.clone())
            .map_err(|e| PocketBridgeError::AdapterError(format!("Invalid DataAnalyticsParams: {e}")))?;
        let in_size = (parsed.record_count as usize) * 8;
        let out_size = in_size; // Upper bound
        Ok((in_size, out_size))
    }

    fn execute_local(&self, params: &serde_json::Value) -> Result<AdapterExecutionResult, PocketBridgeError> {
        let parsed: DataAnalyticsParams = serde_json::from_value(params.clone())
            .map_err(|e| PocketBridgeError::AdapterError(format!("Invalid DataAnalyticsParams: {e}")))?;

        let t_start = Instant::now();
        let (output_sha256, count_filtered, xor_sum) = execute_analytics(&parsed);
        let duration_ms = t_start.elapsed().as_secs_f64() * 1000.0;

        Ok(AdapterExecutionResult {
            adapter_id: DATA_ANALYTICS_ADAPTER_ID.to_string(),
            status: "succeeded".to_string(),
            duration_ms,
            bytes_processed: (count_filtered as usize) * 8,
            output_sha256,
            summary: format!(
                "Filtered {} of {} records (XOR accum: 0x{:016X}) in {:.2} ms",
                count_filtered, parsed.record_count, xor_sum, duration_ms
            ),
            metadata: Some(serde_json::json!({
                "record_count": parsed.record_count,
                "records_passed": count_filtered,
                "xor_accum": format!("0x{:016X}", xor_sum),
            })),
        })
    }
}

/// Execute data analytics reduction locally across CPU threads.
pub fn execute_analytics(params: &DataAnalyticsParams) -> (String, u64, u64) {
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let chunk_size = (params.record_count as usize).div_ceil(num_threads);
    let mut thread_buffers = vec![(Vec::new(), 0u64, 0u64); num_threads];

    std::thread::scope(|s| {
        for (t_idx, slot) in thread_buffers.iter_mut().enumerate() {
            let start = (t_idx * chunk_size) as u64;
            let end = (((t_idx + 1) * chunk_size) as u64).min(params.record_count as u64);
            if start >= end {
                continue;
            }

            s.spawn(move || {
                let mut buf = Vec::new();
                let mut passed = 0u64;
                let mut xor_acc = 0u64;

                for i in start..end {
                    let v_raw = params.seed.wrapping_add(i.wrapping_mul(0x517CC1B727220A95))
                        ^ ((i.wrapping_mul(0x9E3779B97F4A7C15)) >> 17);
                    let v = v_raw as i64;

                    if v >= params.filter_min && v <= params.filter_max {
                        buf.extend_from_slice(&v.to_be_bytes());
                        passed += 1;
                        xor_acc ^= v_raw;
                    }
                }
                *slot = (buf, passed, xor_acc);
            });
        }
    });

    let mut hasher = Sha256::new();
    let mut total_passed = 0u64;
    let mut total_xor = 0u64;

    for (buf, passed, xor_acc) in thread_buffers {
        total_passed += passed;
        total_xor ^= xor_acc;
        hasher.update(&buf);
    }

    (format!("{:x}", hasher.finalize()), total_passed, total_xor)
}
