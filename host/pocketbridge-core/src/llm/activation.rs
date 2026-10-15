use serde::{Deserialize, Serialize};

/// Activation wire frame passed between Host (layers 0..K) and Worker (layers K+1..N)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationFrame {
    pub sequence_id: u64,
    pub token_index: usize,
    pub layer_boundary: usize,
    pub dimensions: Vec<usize>,
    pub dtype: String,
    pub tensor_sha256: String,
    pub data_bytes: Vec<u8>,
}

/// Token generation result from the final layer on the worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedTokenResult {
    pub sequence_id: u64,
    pub token_id: u32,
    pub token_text: String,
    pub layer_latency_ms_host: f64,
    pub layer_latency_ms_worker: f64,
    pub activation_transfer_ms: f64,
}
