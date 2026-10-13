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

    /// Automatically calculate optimal host vs worker ratio based on live hardware capability profiles.
    pub fn from_device_profiles(
        host: &crate::profiler::DeviceProfile,
        worker: &crate::profiler::DeviceProfile,
        host_load_factor: f64,
    ) -> (Self, String) {
        // 1. Calculate raw compute score for host (Intel laptop)
        // IPC baseline ~1.0, scaled by available capacity factor (e.g. 1.0 = idle, 0.2 = heavy compile load)
        let host_cores = (host.cpu_cores as f64).max(1.0);
        let host_capacity = host_load_factor.clamp(0.1, 1.0);
        let host_score = host_cores * 2.8 * host_capacity;

        // 2. Calculate raw compute score for worker (Android phone)
        let worker_cores = (worker.cpu_cores as f64).max(1.0);
        
        // Estimate ARM IPC / frequency based on SoC model
        let freq_ghz = if let Some(soc) = &worker.soc {
            let soc_lower = soc.to_lowercase();
            if soc_lower.contains("8750") || soc_lower.contains("elite") || soc_lower.contains("8 gen 4") {
                4.32 // Snapdragon 8 Elite Oryon custom cores
            } else if soc_lower.contains("8 gen 3") || soc_lower.contains("8 gen 2") {
                3.36
            } else if soc_lower.contains("dimensity 9300") || soc_lower.contains("9400") {
                3.40
            } else {
                2.40 // Standard mid-range ARM SoC
            }
        } else if let Some(model) = &worker.device_model {
            let m_lower = model.to_lowercase();
            if m_lower.contains("s938") || m_lower.contains("s25") {
                4.32
            } else if m_lower.contains("s928") || m_lower.contains("s24") {
                3.36
            } else {
                2.40
            }
        } else {
            2.40
        };

        // 3. Thermal & Battery Health Multiplier
        let mut health_multiplier = 1.0;

        // Battery health gate
        if let Some(pct) = worker.battery_percent {
            let is_charging = worker.battery_charging.unwrap_or(false);
            if pct < 20 && !is_charging {
                health_multiplier = 0.05; // Critical battery: do not drain user's phone
            } else if pct < 35 && !is_charging {
                health_multiplier = 0.50; // Low battery: throttle worker partition
            }
        }

        // Thermal throttle gate
        if let Some(thermal) = &worker.thermal_status {
            match thermal.to_lowercase().as_str() {
                "severe" | "critical" => health_multiplier = 0.05,
                "moderate" => health_multiplier *= 0.50,
                "light" => health_multiplier *= 0.85,
                _ => {}
            }
        }

        let worker_score = worker_cores * freq_ghz * health_multiplier;

        // 4. Compute proportional partition ratio
        let total_score = host_score + worker_score;
        let calculated_host_ratio = (host_score / total_score).clamp(0.10, 0.95);

        let rationale = format!(
            "Dynamic partition calibrated: Host score {:.1} ({} cores @ {:.0}% cap), Phone score {:.1} ({} cores, {:.2} GHz, health multiplier {:.2}) -> {:.1}% Host / {:.1}% Phone",
            host_score, host.cpu_cores, host_capacity * 100.0,
            worker_score, worker.cpu_cores, freq_ghz, health_multiplier,
            calculated_host_ratio * 100.0, (1.0 - calculated_host_ratio) * 100.0
        );

        (Self::new(calculated_host_ratio), rationale)
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
        "ai_whisper.v1" => {
            let total_samples = params.get("samples_count")
                .or_else(|| params.get("records"))
                .and_then(|v| v.as_u64())
                .unwrap_or(160_000);
            let host_samples = ((total_samples as f64) * ratio).round() as u64;
            let worker_samples = total_samples.saturating_sub(host_samples);

            let mut host_p = params.clone();
            let mut worker_p = params.clone();
            if let Some(obj) = host_p.as_object_mut() {
                obj.insert("samples_count".to_string(), serde_json::json!(host_samples));
            }
            if let Some(obj) = worker_p.as_object_mut() {
                obj.insert("samples_count".to_string(), serde_json::json!(worker_samples));
            }
            Ok((host_p, worker_p))
        }
        "browser_cdp.v1" => {
            let total_tabs = params.get("total_tabs")
                .or_else(|| params.get("records"))
                .and_then(|v| v.as_u64())
                .unwrap_or(12);
            let host_tabs = ((total_tabs as f64) * ratio).round().max(1.0) as u64;
            let worker_tabs = total_tabs.saturating_sub(host_tabs);

            let mut host_p = params.clone();
            let mut worker_p = params.clone();
            if let Some(obj) = host_p.as_object_mut() {
                obj.insert("total_tabs".to_string(), serde_json::json!(host_tabs));
                obj.insert("active_tabs".to_string(), serde_json::json!(host_tabs));
            }
            if let Some(obj) = worker_p.as_object_mut() {
                obj.insert("total_tabs".to_string(), serde_json::json!(worker_tabs));
                obj.insert("active_tabs".to_string(), serde_json::json!(0));
            }
            Ok((host_p, worker_p))
        }
        "media_ffmpeg.v1" => {
            let total_frames = params.get("frame_count")
                .or_else(|| params.get("records"))
                .and_then(|v| v.as_u64())
                .unwrap_or(600);
            let host_frames = ((total_frames as f64) * ratio).round() as u64;
            let worker_frames = total_frames.saturating_sub(host_frames);

            let mut host_p = params.clone();
            let mut worker_p = params.clone();
            if let Some(obj) = host_p.as_object_mut() {
                obj.insert("frame_count".to_string(), serde_json::json!(host_frames));
            }
            if let Some(obj) = worker_p.as_object_mut() {
                obj.insert("frame_count".to_string(), serde_json::json!(worker_frames));
            }
            Ok((host_p, worker_p))
        }
        "phone_cluster.v1" => {
            let total_records = params.get("records")
                .or_else(|| params.get("record_count"))
                .and_then(|v| v.as_u64())
                .unwrap_or(100_000);
            let host_records = ((total_records as f64) * ratio).round() as u64;
            let worker_records = total_records.saturating_sub(host_records);

            let mut host_p = params.clone();
            let mut worker_p = params.clone();
            if let Some(obj) = host_p.as_object_mut() {
                obj.insert("records".to_string(), serde_json::json!(host_records));
            }
            if let Some(obj) = worker_p.as_object_mut() {
                obj.insert("records".to_string(), serde_json::json!(worker_records));
            }
            Ok((host_p, worker_p))
        }
        _ => Err(PocketBridgeError::AdapterError(format!(
            "Unsupported adapter for hybrid co-processing: {adapter_id}"
        ))),
    }
}
