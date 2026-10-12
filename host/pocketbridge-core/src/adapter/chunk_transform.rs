use std::time::Instant;
use sha2::{Digest, Sha256};
use super::types::{ChunkTransformParams, TransformOutput};

pub const ADAPTER_ID: &str = "chunk_transform.v1";

/// Compute a single deterministic transformed 64-bit value for an index.
#[inline(always)]
pub fn transform_record(index: u64, seed: u64, rounds: u32) -> u64 {
    // 64-bit linear PRNG step
    let mut state = seed.wrapping_add(index.wrapping_mul(0x9E3779B97F4A7C15));
    for _ in 0..rounds {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state = state.wrapping_mul(0x2545F4914F6CDD1D);
        state = state.rotate_left(13);
    }
    state
}

/// Execute chunk_transform over all records and compute the final SHA-256 digest.
pub fn execute_chunk_transform(params: &ChunkTransformParams) -> TransformOutput {
    let t_start = Instant::now();
    let mut hasher = Sha256::new();

    for i in 0..params.record_count {
        let val = transform_record(i as u64, params.seed, params.rounds);
        hasher.update(&val.to_be_bytes());
    }

    let hash_result = hasher.finalize();
    let output_sha256 = format!("{:x}", hash_result);
    let duration_ms = t_start.elapsed().as_secs_f64() * 1000.0;
    let output_size_bytes = (params.record_count as usize) * 8;

    TransformOutput {
        records_processed: params.record_count as u64,
        rounds: params.rounds,
        output_size_bytes,
        output_sha256,
        duration_ms,
    }
}
