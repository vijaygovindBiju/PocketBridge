use std::collections::{HashMap, VecDeque};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkChunk {
    pub chunk_id: u32,
    pub adapter_id: String,
    pub parameters: serde_json::Value,
    pub records_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkResult {
    pub chunk_id: u32,
    pub worker_serial: String,
    pub output_sha256: String,
    pub duration_ms: f64,
    pub records_processed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetExecutionSummary {
    pub adapter_id: String,
    pub total_chunks: usize,
    pub total_records: u64,
    pub wall_clock_duration_ms: f64,
    pub combined_sha256: String,
    pub worker_chunk_counts: HashMap<String, u32>,
    pub throughput_records_sec: f64,
    pub summary: String,
}

#[derive(Default)]
pub struct FleetWorkQueue {
    pending: VecDeque<WorkChunk>,
    in_flight: HashMap<u32, (String, WorkChunk)>,
    completed: Vec<ChunkResult>,
}

impl FleetWorkQueue {
    pub fn new() -> Self {
        Self {
            pending: VecDeque::new(),
            in_flight: HashMap::new(),
            completed: Vec::new(),
        }
    }

    pub fn push_chunk(&mut self, chunk: WorkChunk) {
        self.pending.push_back(chunk);
    }

    pub fn steal_next(&mut self, worker_serial: &str) -> Option<WorkChunk> {
        let chunk = self.pending.pop_front()?;
        self.in_flight
            .insert(chunk.chunk_id, (worker_serial.to_string(), chunk.clone()));
        Some(chunk)
    }

    pub fn complete_chunk(&mut self, chunk_id: u32, result: ChunkResult) {
        self.in_flight.remove(&chunk_id);
        self.completed.push(result);
    }

    pub fn fail_and_requeue(&mut self, chunk_id: u32) {
        if let Some((_, chunk)) = self.in_flight.remove(&chunk_id) {
            self.pending.push_front(chunk);
        }
    }

    /// When a node unplugs or disconnects, re-queue all of its in-flight micro-chunks immediately.
    pub fn fail_worker_chunks(&mut self, worker_serial: &str) -> usize {
        let to_requeue: Vec<u32> = self
            .in_flight
            .iter()
            .filter(|(_, (serial, _))| serial == worker_serial)
            .map(|(&id, _)| id)
            .collect();

        let count = to_requeue.len();
        for id in to_requeue {
            self.fail_and_requeue(id);
        }
        count
    }

    pub fn is_complete(&self) -> bool {
        self.pending.is_empty() && self.in_flight.is_empty()
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    pub fn aggregate_results(&self, wall_clock_ms: f64) -> FleetExecutionSummary {
        let mut hasher = Sha256::new();
        let mut total_records = 0u64;
        let mut worker_counts: HashMap<String, u32> = HashMap::new();

        let mut sorted = self.completed.clone();
        sorted.sort_by_key(|c| c.chunk_id);

        let adapter_id = sorted
            .first()
            .map(|c| c.chunk_id)
            .map(|_| "distributed_workload")
            .unwrap_or("unknown")
            .to_string();

        for c in &sorted {
            hasher.update(c.output_sha256.as_bytes());
            total_records += c.records_processed;
            *worker_counts.entry(c.worker_serial.clone()).or_insert(0) += 1;
        }

        let combined_sha256 = format!("{:x}", hasher.finalize());
        let throughput = if wall_clock_ms > 0.0 {
            (total_records as f64) / (wall_clock_ms / 1000.0)
        } else {
            0.0
        };

        FleetExecutionSummary {
            adapter_id,
            total_chunks: sorted.len(),
            total_records,
            wall_clock_duration_ms: wall_clock_ms,
            combined_sha256,
            worker_chunk_counts: worker_counts,
            throughput_records_sec: throughput,
            summary: format!(
                "Fleet processed {} chunks ({} records) in {:.2} ms ({:.0} records/sec)",
                sorted.len(),
                total_records,
                wall_clock_ms,
                throughput
            ),
        }
    }
}
