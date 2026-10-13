use thiserror::Error;

#[derive(Error, Debug)]
pub enum PocketBridgeError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Message too large: {size} bytes (max {max})")]
    MessageTooLarge { size: u32, max: u32 },

    #[error("Protocol version mismatch: got {got}, expected {expected}")]
    VersionMismatch { got: u32, expected: u32 },

    #[error("Unexpected message kind: expected {expected}, got {got}")]
    UnexpectedMessage {
        expected: String,
        got: String,
    },

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("ADB error: {0}")]
    AdbError(String),

    #[error("Timeout after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Device profiling error: {0}")]
    ProfilingError(String),

    #[error("Benchmark error: {0}")]
    BenchmarkError(String),

    #[error("Adapter error: {0}")]
    AdapterError(String),

    #[error("Co-processor error: {0}")]
    CoProcessorError(String),

    #[error("Fleet error: {0}")]
    FleetError(String),

    #[error("Plugin error: {0}")]
    PluginError(String),
}
