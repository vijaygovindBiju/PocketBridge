use super::types::CostEstimate;
use serde::{Deserialize, Serialize};

/// Calibrated hardware performance baseline profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceBaseline {
    /// Host compute rate in mathematical operations per second (all available threads)
    pub host_ops_per_sec: f64,
    /// Phone compute rate in mathematical operations per second (all available cores)
    pub phone_ops_per_sec: f64,
    /// USB-C ADB measured link bandwidth in bytes per second
    pub link_bandwidth_bytes_per_sec: f64,
    /// Round-trip ping latency over USB-C in milliseconds
    pub link_rtt_ms: f64,
}

impl Default for PerformanceBaseline {
    /// Default baseline calibrated from Arch Linux (i5-13420H) and Samsung Galaxy S25 Ultra (Snapdragon 8 Elite).
    fn default() -> Self {
        Self {
            // ~32M ops/sec on 12-thread i5-13420H (2M records * 50 rounds = 100M ops in 62ms => ~1.6B loop ops/sec or 32M records/sec)
            host_ops_per_sec: 32_170_000.0,
            // ~8.2M records/sec on 8-core Snapdragon 8 Elite in Kotlin (2M records in 243ms)
            phone_ops_per_sec: 8_230_000.0,
            // Measured ~46.25 MB/s (370 Mbps) sustained throughput over USB-C ADB
            link_bandwidth_bytes_per_sec: 46_250_000.0,
            // Measured round-trip latency over USB-C tunnel
            link_rtt_ms: 2.0,
        }
    }
}

/// Analytical cost estimator for task offloading decisions.
#[derive(Debug, Clone)]
pub struct CostModel {
    pub baseline: PerformanceBaseline,
}

impl Default for CostModel {
    fn default() -> Self {
        Self::new(PerformanceBaseline::default())
    }
}

impl CostModel {
    pub fn new(baseline: PerformanceBaseline) -> Self {
        Self { baseline }
    }

    /// Update the baseline bandwidth from an empirical benchmark result.
    pub fn update_link_bandwidth(&mut self, bandwidth_bytes_per_sec: f64) {
        if bandwidth_bytes_per_sec > 0.0 {
            self.baseline.link_bandwidth_bytes_per_sec = bandwidth_bytes_per_sec;
        }
    }

    /// Update the baseline round-trip latency from an empirical ping measurement.
    pub fn update_link_rtt(&mut self, rtt_ms: f64) {
        if rtt_ms > 0.0 {
            self.baseline.link_rtt_ms = rtt_ms;
        }
    }

    /// Estimate execution costs for a chunk_transform workload.
    ///
    /// # Parameters:
    /// - `record_count`: Number of 64-bit records to transform.
    /// - `rounds`: Mathematical iterations per record.
    /// - `input_bytes`: Size of input payload to transfer.
    /// - `output_bytes`: Expected output payload size.
    /// - `host_load_factor`: Factor in (0.0..=1.0], where 1.0 = host idle, 0.2 = host heavily loaded.
    pub fn estimate_chunk_transform(
        &self,
        record_count: u32,
        rounds: u32,
        input_bytes: usize,
        output_bytes: usize,
        host_load_factor: f64,
    ) -> CostEstimate {
        let load_factor = host_load_factor.clamp(0.05, 1.0);
        let complexity_ratio = (rounds as f64) / 50.0; // Baseline calibrated at 50 rounds
        let effective_records = (record_count as f64) * complexity_ratio;

        // 1. Local Compute Time (ms)
        // Scaled inversely by host_load_factor (if host is at 90% load, host compute takes longer)
        let effective_host_rate = self.baseline.host_ops_per_sec * load_factor;
        let local_compute_ms = (effective_records / effective_host_rate) * 1000.0;

        // 2. Transfer Overhead (ms)
        let bw = self.baseline.link_bandwidth_bytes_per_sec.max(1_000.0);
        let transfer_in_ms = ((input_bytes as f64) / bw) * 1000.0 + (self.baseline.link_rtt_ms / 2.0);
        let transfer_out_ms = ((output_bytes as f64) / bw) * 1000.0 + (self.baseline.link_rtt_ms / 2.0);

        // 3. Remote Compute Time (ms)
        let remote_compute_ms = (effective_records / self.baseline.phone_ops_per_sec.max(1_000.0)) * 1000.0;

        // 4. Remote Total Elapsed (ms)
        let remote_total_ms = transfer_in_ms + remote_compute_ms + transfer_out_ms;

        // 5. Predicted Speedup Ratio
        let predicted_speedup = if remote_total_ms > 0.0 {
            local_compute_ms / remote_total_ms
        } else {
            0.0
        };

        CostEstimate {
            local_compute_ms: local_compute_ms.max(0.01),
            transfer_in_ms: transfer_in_ms.max(0.01),
            transfer_out_ms: transfer_out_ms.max(0.01),
            remote_compute_ms: remote_compute_ms.max(0.01),
            remote_total_ms: remote_total_ms.max(0.02),
            predicted_speedup,
        }
    }
}
