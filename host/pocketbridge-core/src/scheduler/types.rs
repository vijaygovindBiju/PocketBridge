use serde::{Deserialize, Serialize};

/// Target destination recommended by the scheduler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionTarget {
    /// Execute exclusively on the Arch Linux host
    LocalHost,
    /// Offload exclusively to the Android worker over ADB
    RemoteWorker,
    /// Cooperative split: divide batch between host and worker
    HybridSplit {
        local_records: u32,
        remote_records: u32,
    },
}

/// Analytical cost and duration estimates for a workload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostEstimate {
    /// Estimated time to compute locally on host (ms)
    pub local_compute_ms: f64,
    /// Estimated time to transfer input payload over USB-C (ms)
    pub transfer_in_ms: f64,
    /// Estimated time to transfer result payload over USB-C (ms)
    pub transfer_out_ms: f64,
    /// Estimated time to compute remotely on phone worker (ms)
    pub remote_compute_ms: f64,
    /// Total remote wall-clock duration = transfer + remote compute (ms)
    pub remote_total_ms: f64,
    /// Predicted speedup ratio: local_compute_ms / remote_total_ms
    pub predicted_speedup: f64,
}

/// Evaluation of hardware safety and operational constraints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstraintEvaluation {
    /// Battery level is sufficient (or device is actively charging)
    pub battery_ok: bool,
    pub battery_percent: Option<u8>,
    pub battery_charging: Option<bool>,

    /// Thermal status is acceptable (not SEVERE or CRITICAL)
    pub thermal_ok: bool,
    pub thermal_status: Option<String>,

    /// Phone has enough free RAM for the input and working buffers
    pub memory_ok: bool,
    pub memory_required_bytes: u64,
    pub memory_available_bytes: Option<u64>,

    /// USB-C ADB link is active and latency is within acceptable threshold
    pub link_ok: bool,
    pub link_rtt_ms: Option<f64>,
}

impl ConstraintEvaluation {
    /// Returns true if all safety constraints are satisfied.
    pub fn all_passed(&self) -> bool {
        self.battery_ok && self.thermal_ok && self.memory_ok && self.link_ok
    }
}

/// The final explainable scheduling decision produced by the scheduler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchedulingDecision {
    /// Recommended destination
    pub target: ExecutionTarget,
    /// Whether offloading was approved by policy and constraints
    pub approved_for_offload: bool,
    /// Detailed analytical duration estimates
    pub estimates: CostEstimate,
    /// Hardware constraint checks
    pub constraints: ConstraintEvaluation,
    /// Human-readable explanation justifying the decision
    pub decision_reason: String,
    /// Policy speedup margin applied (e.g. 1.15 = 15% minimum speedup required)
    pub speedup_margin: f64,
}

/// Configuration parameters for the explainable scheduler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchedulerConfig {
    /// Required speedup multiplier to justify offload (e.g. 1.15 = 15% faster)
    pub speedup_margin: f64,
    /// Minimum phone battery percentage required when discharging
    pub min_battery_percent: u8,
    /// Automatically reject offload if phone reports SEVERE thermal status
    pub reject_severe_thermal: bool,
    /// Maximum acceptable ping latency over ADB (ms)
    pub max_acceptable_rtt_ms: f64,
    /// Factor reflecting host CPU load (1.0 = idle, 0.2 = host heavily loaded by games/compiles)
    pub host_load_factor: f64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            speedup_margin: 1.15,
            min_battery_percent: 20,
            reject_severe_thermal: true,
            max_acceptable_rtt_ms: 250.0,
            host_load_factor: 1.0,
        }
    }
}
