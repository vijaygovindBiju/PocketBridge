use serde::{Deserialize, Serialize};
use crate::profiler::DeviceProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub payload_sizes: Vec<usize>,
    pub runs_per_size: u32,
    pub include_latency_pings: bool,
    pub latency_ping_sizes: Vec<usize>,
    pub priority: Option<u8>,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            payload_sizes: vec![
                1_000_000,    // 1 MB
                10_000_000,   // 10 MB
                100_000_000,  // 100 MB
            ],
            runs_per_size: 20,
            include_latency_pings: true,
            latency_ping_sizes: vec![64, 1024], // 64 B and 1 KB
            priority: Some(1),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_profile: Option<DeviceProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_profile: Option<DeviceProfile>,
    pub results: Vec<PayloadSizeResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadSizeResult {
    pub payload_size_bytes: usize,
    pub test_type: TestType,
    pub runs: u32,
    pub stats: BenchmarkStats,
    pub samples: Vec<SampleResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestType {
    LatencyPing,
    ThroughputEcho,
}

impl std::fmt::Display for TestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestType::LatencyPing => write!(f, "Latency Ping"),
            TestType::ThroughputEcho => write!(f, "Throughput Echo"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkStats {
    pub median_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub stddev_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub throughput_mbps: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleResult {
    pub sequence: u32,
    pub round_trip_ms: f64,
    pub worker_processing_ms: f64,
    pub payload_size_bytes: usize,
}

pub fn calculate_stats(samples: &[SampleResult], payload_size: usize, is_throughput: bool) -> BenchmarkStats {
    if samples.is_empty() {
        return BenchmarkStats {
            median_ms: 0.0,
            p95_ms: 0.0,
            p99_ms: 0.0,
            min_ms: 0.0,
            max_ms: 0.0,
            stddev_ms: 0.0,
            throughput_mbps: None,
        };
    }

    let mut times: Vec<f64> = samples.iter().map(|s| s.round_trip_ms).collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let count = times.len();
    let min_ms = times[0];
    let max_ms = times[count - 1];

    let median_ms = if count % 2 == 1 {
        times[count / 2]
    } else {
        (times[count / 2 - 1] + times[count / 2]) / 2.0
    };

    let p95_idx = ((count as f64 * 0.95).ceil() as usize).saturating_sub(1).min(count - 1);
    let p99_idx = ((count as f64 * 0.99).ceil() as usize).saturating_sub(1).min(count - 1);
    let p95_ms = times[p95_idx];
    let p99_ms = times[p99_idx];

    let mean = times.iter().sum::<f64>() / count as f64;
    let variance = times.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / count as f64;
    let stddev_ms = variance.sqrt();

    let throughput_mbps = if is_throughput && median_ms > 0.0 {
        // Round trip transfers: sent bytes + received bytes = 2 * payload_size
        let total_bits = payload_size as f64 * 8.0 * 2.0;
        let seconds = median_ms / 1000.0;
        let mbps = (total_bits / seconds) / 1_000_000.0;
        Some(mbps)
    } else {
        None
    };

    BenchmarkStats {
        median_ms,
        p95_ms,
        p99_ms,
        min_ms,
        max_ms,
        stddev_ms,
        throughput_mbps,
    }
}
