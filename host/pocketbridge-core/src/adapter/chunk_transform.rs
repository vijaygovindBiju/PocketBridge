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
/// Utilizes all available CPU cores on the host for realistic multi-core comparison.
pub fn execute_chunk_transform(params: &ChunkTransformParams) -> TransformOutput {
    let t_start = Instant::now();
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let output_sha256 = if params.record_count < 10_000 || num_threads <= 1 {
        // Lightweight sequential path for tiny batches
        let mut hasher = Sha256::new();
        for i in 0..params.record_count {
            let val = transform_record(i as u64, params.seed, params.rounds);
            hasher.update(val.to_be_bytes());
        }
        format!("{:x}", hasher.finalize())
    } else {
        // Full multi-core parallel execution across all host threads
        let chunk_size = (params.record_count as usize).div_ceil(num_threads);
        let mut chunk_results = vec![Vec::new(); num_threads];

        std::thread::scope(|s| {
            for (t_idx, chunk_buf) in chunk_results.iter_mut().enumerate() {
                let start = (t_idx * chunk_size) as u64;
                let end = (((t_idx + 1) * chunk_size) as u64).min(params.record_count as u64);
                if start >= end {
                    continue;
                }
                s.spawn(move || {
                    let mut buf = Vec::with_capacity(((end - start) * 8) as usize);
                    for i in start..end {
                        let val = transform_record(i, params.seed, params.rounds);
                        buf.extend_from_slice(&val.to_be_bytes());
                    }
                    *chunk_buf = buf;
                });
            }
        });

        let mut hasher = Sha256::new();
        for chunk in chunk_results {
            hasher.update(&chunk);
        }
        format!("{:x}", hasher.finalize())
    };

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
