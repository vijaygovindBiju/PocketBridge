use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Protocol envelope wrapping every control message.
/// Follows PROTOCOL.md §2 with added priority support.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub protocol_version: u32,
    pub message_id: String,
    pub session_id: String,
    pub kind: MessageKind,
    pub sent_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    pub payload: serde_json::Value,
}

impl Envelope {
    pub fn new(kind: MessageKind, payload: serde_json::Value) -> Self {
        Self {
            protocol_version: 1,
            message_id: Uuid::new_v4().to_string(),
            session_id: String::new(),
            kind,
            sent_at_unix_ms: current_unix_ms(),
            priority: None,
            payload,
        }
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = Some(priority);
        self
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = session_id.into();
        self
    }
}

pub fn current_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// All message kinds from PROTOCOL.md §3 and benchmark extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MessageKind {
    Hello,
    AuthChallenge,
    AuthProof,
    Capabilities,
    Heartbeat,
    SubmitJob,
    JobAccepted,
    JobRejected,
    InputChunk,
    InputComplete,
    JobStarted,
    JobProgress,
    CancelJob,
    Cancelled,
    ResultManifest,
    ResultChunk,
    ResultComplete,
    JobError,
    Goodbye,
    BenchmarkEcho,
    BenchmarkEchoReply,
}

impl std::fmt::Display for MessageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
