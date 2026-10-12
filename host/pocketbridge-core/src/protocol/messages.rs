use serde::{Deserialize, Serialize};
use super::envelope::{Envelope, MessageKind};

/// HELLO payload — protocol version negotiation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HelloPayload {
    pub agent_name: String,
    pub agent_version: String,
    pub protocol_version_min: u32,
    pub protocol_version_max: u32,
    pub role: AgentRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Host,
    Worker,
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRole::Host => write!(f, "host"),
            AgentRole::Worker => write!(f, "worker"),
        }
    }
}

/// HEARTBEAT payload — liveness + resource summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeartbeatPayload {
    pub uptime_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_usage_percent: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_used_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_total_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature_celsius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery_percent: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery_charging: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thermal_status: Option<String>,
}

/// BENCHMARK_ECHO metadata payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkEchoPayload {
    pub sequence: u32,
    pub binary_size_bytes: usize,
    pub sent_at_unix_ms: u64,
}

/// BENCHMARK_ECHO_REPLY metadata payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkEchoReplyPayload {
    pub sequence: u32,
    pub binary_size_bytes: usize,
    pub original_sent_at_unix_ms: u64,
    pub worker_received_at_unix_ms: u64,
    pub worker_replied_at_unix_ms: u64,
}

// ----------------------------------------------------------------------------
// Job Lifecycle Protocol Payloads (Phase 1 & Phase 2)
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobRequirements {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    pub min_memory_bytes: u64,
    pub max_wall_time_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobLimits {
    pub max_input_bytes: u64,
    pub max_output_bytes: u64,
    pub network_access: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubmitJobPayload {
    pub job_id: String,
    pub adapter_id: String,
    pub input_parameters: serde_json::Value,
    pub requirements: JobRequirements,
    pub limits: JobLimits,
    pub deadline_unix_ms: u64,
    pub idempotent: bool,
    pub priority: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobAcceptedPayload {
    pub job_id: String,
    pub attempt_id: String,
    pub accepted_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobRejectedPayload {
    pub job_id: String,
    pub reason: String,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobStartedPayload {
    pub job_id: String,
    pub started_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobProgressPayload {
    pub job_id: String,
    pub progress_fraction: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_used_bytes: Option<u64>,
    pub elapsed_compute_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CancelJobPayload {
    pub job_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobCancelledPayload {
    pub job_id: String,
    pub cancelled_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultManifestPayload {
    pub job_id: String,
    pub adapter_id: String,
    pub status: String, // "succeeded" or "failed"
    pub output_size_bytes: usize,
    pub output_sha256: String,
    pub compute_duration_ms: u64,
    pub records_processed: u64,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobErrorPayload {
    pub job_id: String,
    pub error_code: String,
    pub message: String,
}

// ----------------------------------------------------------------------------
// Helper constructors on Envelope
// ----------------------------------------------------------------------------

impl Envelope {
    pub fn hello(agent_name: impl Into<String>, agent_version: impl Into<String>, role: AgentRole) -> Self {
        let payload = serde_json::to_value(HelloPayload {
            agent_name: agent_name.into(),
            agent_version: agent_version.into(),
            protocol_version_min: 1,
            protocol_version_max: 1,
            role,
        }).expect("valid HelloPayload JSON");
        Envelope::new(MessageKind::Hello, payload)
    }

    pub fn heartbeat(payload: HeartbeatPayload) -> Self {
        let val = serde_json::to_value(payload).expect("valid HeartbeatPayload JSON");
        Envelope::new(MessageKind::Heartbeat, val)
    }

    pub fn benchmark_echo(sequence: u32, binary_size_bytes: usize, sent_at_unix_ms: u64) -> Self {
        let val = serde_json::to_value(BenchmarkEchoPayload {
            sequence,
            binary_size_bytes,
            sent_at_unix_ms,
        }).expect("valid BenchmarkEchoPayload JSON");
        Envelope::new(MessageKind::BenchmarkEcho, val)
    }

    pub fn benchmark_echo_reply(
        sequence: u32,
        binary_size_bytes: usize,
        original_sent_at_unix_ms: u64,
        worker_received_at_unix_ms: u64,
        worker_replied_at_unix_ms: u64,
    ) -> Self {
        let val = serde_json::to_value(BenchmarkEchoReplyPayload {
            sequence,
            binary_size_bytes,
            original_sent_at_unix_ms,
            worker_received_at_unix_ms,
            worker_replied_at_unix_ms,
        }).expect("valid BenchmarkEchoReplyPayload JSON");
        Envelope::new(MessageKind::BenchmarkEchoReply, val)
    }

    pub fn submit_job(payload: SubmitJobPayload) -> Self {
        let priority = payload.priority;
        let val = serde_json::to_value(payload).expect("valid SubmitJobPayload JSON");
        Envelope::new(MessageKind::SubmitJob, val).with_priority(priority)
    }

    pub fn job_accepted(job_id: impl Into<String>, attempt_id: impl Into<String>, accepted_at: u64) -> Self {
        let val = serde_json::to_value(JobAcceptedPayload {
            job_id: job_id.into(),
            attempt_id: attempt_id.into(),
            accepted_at_unix_ms: accepted_at,
        }).expect("valid JobAcceptedPayload JSON");
        Envelope::new(MessageKind::JobAccepted, val)
    }

    pub fn job_rejected(job_id: impl Into<String>, reason: impl Into<String>, code: impl Into<String>) -> Self {
        let val = serde_json::to_value(JobRejectedPayload {
            job_id: job_id.into(),
            reason: reason.into(),
            code: code.into(),
        }).expect("valid JobRejectedPayload JSON");
        Envelope::new(MessageKind::JobRejected, val)
    }

    pub fn job_started(job_id: impl Into<String>, started_at: u64) -> Self {
        let val = serde_json::to_value(JobStartedPayload {
            job_id: job_id.into(),
            started_at_unix_ms: started_at,
        }).expect("valid JobStartedPayload JSON");
        Envelope::new(MessageKind::JobStarted, val)
    }

    pub fn job_progress(payload: JobProgressPayload) -> Self {
        let val = serde_json::to_value(payload).expect("valid JobProgressPayload JSON");
        Envelope::new(MessageKind::JobProgress, val)
    }

    pub fn cancel_job(job_id: impl Into<String>, reason: impl Into<String>) -> Self {
        let val = serde_json::to_value(CancelJobPayload {
            job_id: job_id.into(),
            reason: reason.into(),
        }).expect("valid CancelJobPayload JSON");
        Envelope::new(MessageKind::CancelJob, val)
    }

    pub fn job_cancelled(job_id: impl Into<String>, cancelled_at: u64) -> Self {
        let val = serde_json::to_value(JobCancelledPayload {
            job_id: job_id.into(),
            cancelled_at_unix_ms: cancelled_at,
        }).expect("valid JobCancelledPayload JSON");
        Envelope::new(MessageKind::Cancelled, val)
    }

    pub fn result_manifest(payload: ResultManifestPayload) -> Self {
        let val = serde_json::to_value(payload).expect("valid ResultManifestPayload JSON");
        Envelope::new(MessageKind::ResultManifest, val)
    }

    pub fn job_error(job_id: impl Into<String>, error_code: impl Into<String>, message: impl Into<String>) -> Self {
        let val = serde_json::to_value(JobErrorPayload {
            job_id: job_id.into(),
            error_code: error_code.into(),
            message: message.into(),
        }).expect("valid JobErrorPayload JSON");
        Envelope::new(MessageKind::JobError, val)
    }
}
