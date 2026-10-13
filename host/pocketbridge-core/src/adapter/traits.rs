use serde::{Deserialize, Serialize};
use crate::error::PocketBridgeError;

/// Result of an adapter task execution (local or remote).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterExecutionResult {
    pub adapter_id: String,
    pub status: String,
    pub duration_ms: f64,
    pub bytes_processed: usize,
    pub output_sha256: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Generic trait for all task adapters supported by PocketBridge.
pub trait WorkloadAdapter: Send + Sync {
    /// Unique identifier for this adapter (e.g. "chunk_transform.v1", "image_batch.v1").
    fn adapter_id(&self) -> &str;

    /// Human-readable description of what this adapter does.
    fn description(&self) -> &str;

    /// Validate JSON input parameters before submission.
    fn validate_params(&self, params: &serde_json::Value) -> Result<(), PocketBridgeError>;

    /// Estimate memory and payload sizes in bytes for scheduling.
    fn estimate_payload_sizes(&self, params: &serde_json::Value) -> Result<(usize, usize), PocketBridgeError>;

    /// Execute the workload locally on the host.
    fn execute_local(&self, params: &serde_json::Value) -> Result<AdapterExecutionResult, PocketBridgeError>;
}
