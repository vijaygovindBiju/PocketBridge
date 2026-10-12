use serde::{Deserialize, Serialize};
use super::envelope::Envelope;

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
/// Note: raw binary payload of size `binary_size_bytes` immediately follows the envelope in the stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkEchoPayload {
    pub sequence: u32,
    pub binary_size_bytes: usize,
    pub sent_at_unix_ms: u64,
}

/// BENCHMARK_ECHO_REPLY metadata payload.
/// The raw binary bytes are echoed back right after this envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkEchoReplyPayload {
    pub sequence: u32,
    pub binary_size_bytes: usize,
    pub original_sent_at_unix_ms: u64,
    pub worker_received_at_unix_ms: u64,
    pub worker_replied_at_unix_ms: u64,
}

/// Helper constructors for envelopes with typed payloads
impl Envelope {
    pub fn hello(agent_name: impl Into<String>, agent_version: impl Into<String>, role: AgentRole) -> Self {
        let payload = serde_json::to_value(HelloPayload {
            agent_name: agent_name.into(),
            agent_version: agent_version.into(),
            protocol_version_min: 1,
            protocol_version_max: 1,
            role,
        }).expect("valid HelloPayload JSON");
        Envelope::new(super::envelope::MessageKind::Hello, payload)
    }

    pub fn heartbeat(payload: HeartbeatPayload) -> Self {
        let val = serde_json::to_value(payload).expect("valid HeartbeatPayload JSON");
        Envelope::new(super::envelope::MessageKind::Heartbeat, val)
    }

    pub fn benchmark_echo(sequence: u32, binary_size_bytes: usize, sent_at_unix_ms: u64) -> Self {
        let val = serde_json::to_value(BenchmarkEchoPayload {
            sequence,
            binary_size_bytes,
            sent_at_unix_ms,
        }).expect("valid BenchmarkEchoPayload JSON");
        Envelope::new(super::envelope::MessageKind::BenchmarkEcho, val)
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
        Envelope::new(super::envelope::MessageKind::BenchmarkEchoReply, val)
    }
}
