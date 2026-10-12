use super::chunk_transform::{execute_chunk_transform, ADAPTER_ID};
use super::traits::{AdapterExecutionResult, WorkloadAdapter};
use super::types::ChunkTransformParams;
use crate::error::PocketBridgeError;

pub struct ChunkTransformAdapter;

impl WorkloadAdapter for ChunkTransformAdapter {
    fn adapter_id(&self) -> &'static str {
        ADAPTER_ID
    }

    fn description(&self) -> &'static str {
        "Synthetic deterministic 64-bit integer PRNG transform with iterative diffusion"
    }

    fn validate_params(&self, params: &serde_json::Value) -> Result<(), PocketBridgeError> {
        let _parsed: ChunkTransformParams = serde_json::from_value(params.clone())
            .map_err(|e| PocketBridgeError::AdapterError(format!("Invalid ChunkTransformParams: {e}")))?;
        Ok(())
    }

    fn estimate_payload_sizes(&self, params: &serde_json::Value) -> Result<(usize, usize), PocketBridgeError> {
        let parsed: ChunkTransformParams = serde_json::from_value(params.clone())
            .map_err(|e| PocketBridgeError::AdapterError(format!("Invalid ChunkTransformParams: {e}")))?;
        let size = (parsed.record_count as usize) * 8;
        Ok((size, size))
    }

    fn execute_local(&self, params: &serde_json::Value) -> Result<AdapterExecutionResult, PocketBridgeError> {
        let parsed: ChunkTransformParams = serde_json::from_value(params.clone())
            .map_err(|e| PocketBridgeError::AdapterError(format!("Invalid ChunkTransformParams: {e}")))?;

        let out = execute_chunk_transform(&parsed);

        Ok(AdapterExecutionResult {
            adapter_id: ADAPTER_ID.to_string(),
            status: "succeeded".to_string(),
            duration_ms: out.duration_ms,
            bytes_processed: out.output_size_bytes,
            output_sha256: out.output_sha256,
            summary: format!(
                "Processed {} records across {} rounds in {:.2} ms",
                out.records_processed, out.rounds, out.duration_ms
            ),
            metadata: Some(serde_json::json!({
                "records_processed": out.records_processed,
                "rounds": out.rounds,
            })),
        })
    }
}
