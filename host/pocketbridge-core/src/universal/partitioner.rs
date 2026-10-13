use serde::{Deserialize, Serialize};

/// Strategy to partition a generic workload into host and worker slices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartitionStrategy {
    /// Line-delimited stream or text records (e.g., CSV, JSONL, logs)
    LineRecord,
    /// Fixed-size binary byte chunking (e.g., media files, raw streams, archives)
    ByteChunk,
    /// List of discrete items or file paths (e.g., batch media files, compiler units)
    ItemList,
}

/// A generic partition plan dividing any divisible workload between Host and Worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UniversalPartitionPlan {
    pub strategy: PartitionStrategy,
    pub total_units: usize,
    pub host_units: usize,
    pub worker_units: usize,
    pub host_ratio: f64,
}

/// Generic partitioner capable of slicing arbitrary text streams, byte buffers, or argument lists.
pub struct UniversalPartitioner;

impl UniversalPartitioner {
    /// Plan partitioning across total units using capability-calibrated host ratio.
    pub fn plan(strategy: PartitionStrategy, total_units: usize, host_ratio: f64) -> UniversalPartitionPlan {
        let ratio = host_ratio.clamp(0.01, 0.99);
        let host_units = ((total_units as f64) * ratio).round() as usize;
        let worker_units = total_units.saturating_sub(host_units);

        UniversalPartitionPlan {
            strategy,
            total_units,
            host_units,
            worker_units,
            host_ratio: ratio,
        }
    }

    /// Split a line-delimited string (CSV, logs, JSONL) into host and worker slices.
    pub fn partition_lines(input: &str, host_ratio: f64) -> (String, String) {
        let lines: Vec<&str> = input.lines().collect();
        let plan = Self::plan(PartitionStrategy::LineRecord, lines.len(), host_ratio);

        let (host_lines, worker_lines) = lines.split_at(plan.host_units);
        (host_lines.join("\n"), worker_lines.join("\n"))
    }

    /// Split a binary byte buffer (e.g., raw stream, archive) into host and worker slices.
    pub fn partition_bytes(data: &[u8], host_ratio: f64) -> (Vec<u8>, Vec<u8>) {
        let plan = Self::plan(PartitionStrategy::ByteChunk, data.len(), host_ratio);
        let (host_bytes, worker_bytes) = data.split_at(plan.host_units);
        (host_bytes.to_vec(), worker_bytes.to_vec())
    }

    /// Split a list of items or file paths (e.g. `ffmpeg` frame list, compiler targets) into host and worker slices.
    pub fn partition_items<T: Clone>(items: &[T], host_ratio: f64) -> (Vec<T>, Vec<T>) {
        let plan = Self::plan(PartitionStrategy::ItemList, items.len(), host_ratio);
        let (host_items, worker_items) = items.split_at(plan.host_units);
        (host_items.to_vec(), worker_items.to_vec())
    }
}
