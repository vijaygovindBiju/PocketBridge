use serde::{Deserialize, Serialize};

/// Generic workload adapter identifier for universal proxy tasks.
pub const UNIVERSAL_ADAPTER_ID: &str = "universal_exec.v1";

/// Payload sent to execute a generic sliced command or script remotely on Android.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalExecPayload {
    /// Command or task type identifier (e.g., "stream_process", "media_slice", "text_transform")
    pub task_type: String,
    /// Raw payload or parameter data (text or base64)
    pub payload: String,
    /// Strategy used for partitioning
    pub strategy: String,
    /// Assigned chunk index
    pub chunk_index: usize,
    /// Total chunks in this job
    pub total_chunks: usize,
}

/// Result returned from executing a universal task on the worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalExecResult {
    pub chunk_index: usize,
    pub status: String,
    pub output_data: String,
    pub bytes_processed: usize,
    pub duration_ms: f64,
    pub sha256_hash: String,
}
