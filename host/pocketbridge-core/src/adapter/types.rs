use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkTransformParams {
    pub record_count: u32,
    pub rounds: u32,
    pub seed: u64,
}

impl Default for ChunkTransformParams {
    fn default() -> Self {
        Self {
            record_count: 50_000,
            rounds: 20,
            seed: 0x123456789ABCDEF0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformOutput {
    pub records_processed: u64,
    pub rounds: u32,
    pub output_size_bytes: usize,
    pub output_sha256: String,
    pub duration_ms: f64,
}
