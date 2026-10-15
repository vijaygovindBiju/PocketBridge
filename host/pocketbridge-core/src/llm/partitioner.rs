use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use super::gguf::GgufModel;

/// Layer breakdown and memory footprint for a single transformer block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerFootprint {
    pub layer_index: usize,
    pub tensor_count: usize,
    pub weights_bytes: u64,
}

/// Distributed execution layer allocation plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedLayerPlan {
    pub model_name: String,
    pub architecture: String,
    pub total_layers: usize,
    pub total_model_weights_bytes: u64,
    pub embedding_bytes: u64,
    pub output_norm_bytes: u64,
    pub lm_head_bytes: u64,

    // Host allocation (e.g. Layers 0..K)
    pub host_start_layer: usize,
    pub host_end_layer: usize,
    pub host_layers_count: usize,
    pub host_weights_bytes: u64,
    pub host_ram_fits: bool,

    // Phone Worker allocation (e.g. Layers K+1..N-1)
    pub worker_start_layer: usize,
    pub worker_end_layer: usize,
    pub worker_layers_count: usize,
    pub worker_weights_bytes: u64,
    pub worker_ram_fits: bool,

    // Wire transfer characteristics
    pub activation_tensor_bytes_per_token: u64,
    pub activation_transfer_ms_est: f64,
    pub adb_link_bandwidth_mbps: f64,

    // Feasibility & blockers
    pub can_execute_on_host_alone: bool,
    pub can_execute_on_worker_alone: bool,
    pub can_execute_hybrid_distributed: bool,
    pub execution_blockers: Vec<String>,
}

impl DistributedLayerPlan {
    /// Calculate the optimal distributed layer partition plan based on actual GGUF tensors
    /// and device physical memory limits.
    pub fn plan(
        model: &GgufModel,
        host_available_ram_bytes: u64,
        worker_available_ram_bytes: u64,
        adb_link_bandwidth_mbps: f64,
    ) -> Self {
        let mut layer_map: BTreeMap<usize, (usize, u64)> = BTreeMap::new();
        let mut embedding_bytes: u64 = 0;
        let mut output_norm_bytes: u64 = 0;
        let mut lm_head_bytes: u64 = 0;
        let mut total_model_weights_bytes: u64 = 0;

        for t in &model.tensors {
            total_model_weights_bytes += t.estimated_bytes;

            if t.name == "token_embd.weight" {
                embedding_bytes += t.estimated_bytes;
            } else if t.name == "output_norm.weight" {
                output_norm_bytes += t.estimated_bytes;
            } else if t.name == "output.weight" {
                lm_head_bytes += t.estimated_bytes;
            } else if t.name.starts_with("blk.") {
                // Parse layer index from "blk.X...."
                let parts: Vec<&str> = t.name.split('.').collect();
                if parts.len() >= 2 {
                    if let Ok(idx) = parts[1].parse::<usize>() {
                        let entry = layer_map.entry(idx).or_insert((0, 0));
                        entry.0 += 1;
                        entry.1 += t.estimated_bytes;
                    }
                }
            }
        }

        let total_layers = if model.block_count > 0 {
            model.block_count
        } else {
            layer_map.len()
        };

        // Find the optimal partition boundary layer K
        // Host gets: Embedding + Layers 0..=K
        // Worker gets: Layers (K+1)..=(total_layers-1) + OutputNorm + LMHead
        let mut best_split_k = total_layers / 2;
        let mut min_load_diff = u64::MAX;

        for k in 0..total_layers {
            let mut host_bytes = embedding_bytes;
            for i in 0..=k {
                if let Some((_, bytes)) = layer_map.get(&i) {
                    host_bytes += bytes;
                }
            }

            let mut worker_bytes = output_norm_bytes + lm_head_bytes;
            for i in (k + 1)..total_layers {
                if let Some((_, bytes)) = layer_map.get(&i) {
                    worker_bytes += bytes;
                }
            }

            let host_fits = host_bytes <= host_available_ram_bytes;
            let worker_fits = worker_bytes <= worker_available_ram_bytes;

            if host_fits && worker_fits {
                let diff = if host_bytes > worker_bytes { host_bytes - worker_bytes } else { worker_bytes - host_bytes };
                if diff < min_load_diff {
                    min_load_diff = diff;
                    best_split_k = k;
                }
            }
        }

        // Compute final allocations using best_split_k
        let mut host_weights = embedding_bytes;
        for i in 0..=best_split_k {
            if let Some((_, b)) = layer_map.get(&i) { host_weights += b; }
        }

        let mut worker_weights = output_norm_bytes + lm_head_bytes;
        for i in (best_split_k + 1)..total_layers {
            if let Some((_, b)) = layer_map.get(&i) { worker_weights += b; }
        }

        let host_ram_fits = host_weights <= host_available_ram_bytes;
        let worker_ram_fits = worker_weights <= worker_available_ram_bytes;
        let can_execute_on_host_alone = total_model_weights_bytes <= host_available_ram_bytes;
        let can_execute_on_worker_alone = total_model_weights_bytes <= worker_available_ram_bytes;
        let can_execute_hybrid_distributed = host_ram_fits && worker_ram_fits;

        // Activation tensor size per token:
        // In transformer pipeline parallelism, activation passed between layer K and K+1
        // is [1, 1, embedding_length] in FP16 (2 bytes per element).
        let activation_tensor_bytes_per_token = (model.embedding_length.max(4096)) * 2;
        let bytes_per_sec = (adb_link_bandwidth_mbps * 1_000_000.0) / 8.0;
        let activation_transfer_ms_est = if bytes_per_sec > 0.0 {
            (activation_tensor_bytes_per_token as f64 / bytes_per_sec) * 1000.0
        } else {
            1.0
        };

        let mut execution_blockers = Vec::new();
        if !can_execute_on_host_alone {
            execution_blockers.push(format!(
                "Standalone Host OOM: Model weights ({:.2} GB) exceed available laptop RAM ({:.2} GB). Standalone loading will trigger kernel oom-killer.",
                total_model_weights_bytes as f64 / 1e9,
                host_available_ram_bytes as f64 / 1e9
            ));
        }
        if !can_execute_hybrid_distributed {
            if !host_ram_fits {
                execution_blockers.push(format!(
                    "Host partition ({:.2} GB) exceeds available laptop RAM ({:.2} GB).",
                    host_weights as f64 / 1e9, host_available_ram_bytes as f64 / 1e9
                ));
            }
            if !worker_ram_fits {
                execution_blockers.push(format!(
                    "Worker partition ({:.2} GB) exceeds available mobile worker RAM ({:.2} GB).",
                    worker_weights as f64 / 1e9, worker_available_ram_bytes as f64 / 1e9
                ));
            }
        }
        execution_blockers.push(
            "Worker Runtime: Android phone requires an active on-device llama-rpc-server binary listening on forwarded port (tcp:50052) for genuine tensor offload.".to_string()
        );

        Self {
            model_name: model.model_name.clone(),
            architecture: model.architecture.clone(),
            total_layers,
            total_model_weights_bytes,
            embedding_bytes,
            output_norm_bytes,
            lm_head_bytes,
            host_start_layer: 0,
            host_end_layer: best_split_k,
            host_layers_count: best_split_k + 1,
            host_weights_bytes: host_weights,
            host_ram_fits,
            worker_start_layer: best_split_k + 1,
            worker_end_layer: total_layers.saturating_sub(1),
            worker_layers_count: total_layers.saturating_sub(best_split_k + 1),
            worker_weights_bytes: worker_weights,
            worker_ram_fits,
            activation_tensor_bytes_per_token,
            activation_transfer_ms_est,
            adb_link_bandwidth_mbps,
            can_execute_on_host_alone,
            can_execute_on_worker_alone,
            can_execute_hybrid_distributed,
            execution_blockers,
        }
    }
}
