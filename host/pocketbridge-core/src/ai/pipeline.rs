use serde::{Deserialize, Serialize};

pub const AI_PIPELINE_ADAPTER_ID: &str = "ai_pipeline.v1";

/// Configuration for partitioning a Transformer neural network across device boundaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelPipelineConfig {
    pub model_name: String,
    pub total_layers: usize,
    pub host_layers: usize,
    pub worker_layers: usize,
    pub hidden_dimension: usize,
}

impl ModelPipelineConfig {
    /// Calculate layer partition from device capability ratio.
    pub fn new(model_name: &str, total_layers: usize, hidden_dimension: usize, host_ratio: f64) -> Self {
        let ratio = host_ratio.clamp(0.1, 0.9);
        let host_layers = ((total_layers as f64) * ratio).round() as usize;
        let worker_layers = total_layers.saturating_sub(host_layers);

        Self {
            model_name: model_name.to_string(),
            total_layers,
            host_layers,
            worker_layers,
            hidden_dimension,
        }
    }

    /// Size of the intermediate activation tensor transferred across USB per token (in bytes).
    /// Typically float16 (2 bytes per dimension).
    pub fn activation_tensor_bytes(&self) -> usize {
        self.hidden_dimension * 2
    }

    /// Estimated transfer latency over USB 3 link (in milliseconds) for an activation tensor.
    pub fn transfer_latency_ms(&self, throughput_mbps: f64) -> f64 {
        let bytes = self.activation_tensor_bytes() as f64;
        let bits = bytes * 8.0;
        let bits_per_ms = (throughput_mbps * 1_000_000.0) / 1000.0;
        bits / bits_per_ms
    }
}

/// Activation tensor payload transferred between Host layers and Worker layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationTensorPayload {
    pub token_index: usize,
    pub layer_boundary: usize,
    pub tensor_shape: Vec<usize>,
    pub tensor_data_base64: String,
}
