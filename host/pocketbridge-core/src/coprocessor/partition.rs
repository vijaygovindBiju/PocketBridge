use serde::{Deserialize, Serialize};
use crate::adapter::{
    ChunkTransformParams, DataAnalyticsParams, ImageBatchParams,
    ADAPTER_ID, DATA_ANALYTICS_ADAPTER_ID, IMAGE_BATCH_ADAPTER_ID,
};
use crate::error::PocketBridgeError;

/// Configuration for workload partitioning between Host (laptop) and Worker (phone).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PartitionConfig {
    /// Fraction of workload assigned to the host (0.01 to 0.99, e.g. 0.5 = 50% host, 50% phone)
    pub host_ratio: f64,
}

impl Default for PartitionConfig {
    fn default() -> Self {
        Self { host_ratio: 0.5 }
    }
}

impl PartitionConfig {
    pub fn new(host_ratio: f64) -> Self {
        Self {
            host_ratio: host_ratio.clamp(0.01, 0.99),
        }
    }
}

/// Partition an adapter workload's parameters into host slice and worker slice.
pub fn partition_workload(
    adapter_id: &str,
    params: &serde_json::Value,
    host_ratio: f64,
) -> Result<(serde_json::Value, serde_json::Value), PocketBridgeError> {
    let ratio = host_ratio.clamp(0.01, 0.99);

    match adapter_id {
        ADAPTER_ID => {
            let p: ChunkTransformParams = serde_json::from_value(params.clone())
                .map_err(|e| PocketBridgeError::AdapterError(format!("Invalid ChunkTransformParams: {e}")))?;
            let host_count = ((p.record_count as f64) * ratio).round() as u32;
            let worker_count = p.record_count.saturating_sub(host_count);

            let host_params = ChunkTransformParams {
                record_count: host_count,
                rounds: p.rounds,
                seed: p.seed,
            };
            let worker_params = ChunkTransformParams {
                record_count: worker_count,
                rounds: p.rounds,
                seed: p.seed.wrapping_add(host_count as u64),
            };

            Ok((
                serde_json::to_value(host_params)?,
                serde_json::to_value(worker_params)?,
            ))
        }
        IMAGE_BATCH_ADAPTER_ID => {
            let p: ImageBatchParams = serde_json::from_value(params.clone())
                .map_err(|e| PocketBridgeError::AdapterError(format!("Invalid ImageBatchParams: {e}")))?;
            let host_count = ((p.image_count as f64) * ratio).round() as u32;
            let worker_count = p.image_count.saturating_sub(host_count);

            let host_params = ImageBatchParams {
                image_count: host_count,
                width: p.width,
                height: p.height,
                seed: p.seed,
                operation: p.operation.clone(),
            };
            let worker_params = ImageBatchParams {
                image_count: worker_count,
                width: p.width,
                height: p.height,
                seed: p.seed.wrapping_add(host_count as u64),
                operation: p.operation,
            };

            Ok((
                serde_json::to_value(host_params)?,
                serde_json::to_value(worker_params)?,
            ))
        }
        DATA_ANALYTICS_ADAPTER_ID => {
            let p: DataAnalyticsParams = serde_json::from_value(params.clone())
                .map_err(|e| PocketBridgeError::AdapterError(format!("Invalid DataAnalyticsParams: {e}")))?;
            let host_count = ((p.record_count as f64) * ratio).round() as u32;
            let worker_count = p.record_count.saturating_sub(host_count);

            let host_params = DataAnalyticsParams {
                record_count: host_count,
                seed: p.seed,
                filter_min: p.filter_min,
                filter_max: p.filter_max,
            };
            let worker_params = DataAnalyticsParams {
                record_count: worker_count,
                seed: p.seed.wrapping_add(host_count as u64),
                filter_min: p.filter_min,
                filter_max: p.filter_max,
            };

            Ok((
                serde_json::to_value(host_params)?,
                serde_json::to_value(worker_params)?,
            ))
        }
        _ => Err(PocketBridgeError::AdapterError(format!(
            "Unsupported adapter for hybrid co-processing: {adapter_id}"
        ))),
    }
}
