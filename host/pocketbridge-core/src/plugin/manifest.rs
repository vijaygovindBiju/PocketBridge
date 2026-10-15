use serde::{Deserialize, Serialize};

/// Hardware and runtime requirements for a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRequirements {
    /// Minimum host (laptop) RAM required in megabytes.
    pub min_host_ram_mb: u64,
    /// Minimum worker (phone) RAM required in megabytes.
    pub min_worker_ram_mb: u64,
    /// Minimum phone storage required in megabytes.
    pub min_worker_storage_mb: u64,
}

impl Default for PluginRequirements {
    fn default() -> Self {
        Self {
            min_host_ram_mb: 128,
            min_worker_ram_mb: 64,
            min_worker_storage_mb: 10,
        }
    }
}

/// Category / architecture kind of the plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginKind {
    /// Native compiled shared object (.so) or static Rust/C++ module.
    Native,
    /// Script or executable worker (e.g. Python, Node, Shell).
    Script,
    /// Machine learning model weights (GGUF, ONNX, TFLite) and compute pipeline.
    AiModel,
    /// Headless Chromium or WebView rendering engine.
    WebHeadless,
}

/// Strict, non-interchangeable capability classification for workloads and plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    /// Functionality is implemented and executable in code.
    Implemented,
    /// Functionality has been tested on physical hardware (e.g. Android phone + Linux laptop).
    HardwareTested,
    /// Functionality has reproducible empirical benchmark results and validation.
    BenchmarkVerified,
    /// Implemented but still experimental / subject to validation.
    Experimental,
    /// Execution is simulated or a synthetic stress kernel; not real production processing.
    Simulated,
    /// Partial scaffolding without an operational end-to-end execution path.
    Scaffolded,
    /// Planned roadmap capability; not implemented yet.
    Planned,
}

impl Default for CapabilityStatus {
    fn default() -> Self {
        Self::Experimental
    }
}

impl std::fmt::Display for CapabilityStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Implemented => write!(f, "Implemented"),
            Self::HardwareTested => write!(f, "Hardware-tested"),
            Self::BenchmarkVerified => write!(f, "Benchmark-verified"),
            Self::Experimental => write!(f, "Experimental"),
            Self::Simulated => write!(f, "Simulated"),
            Self::Scaffolded => write!(f, "Scaffolded"),
            Self::Planned => write!(f, "Planned"),
        }
    }
}

/// Metadata and execution manifest for a PocketBridge plugin package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Unique identifier for the plugin (e.g., "image-batch", "ai-whisper").
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Semantic version (e.g., "1.0.0").
    pub version: String,
    /// Descriptive summary of what the plugin does.
    pub description: String,
    /// Author or maintainer attribution.
    pub author: String,
    /// Software license (e.g., "Apache-2.0", "MIT").
    pub license: String,
    /// Type of plugin engine.
    pub kind: PluginKind,
    /// The workload adapter identifier registered in the system (e.g., "image_batch.v1").
    pub adapter_id: String,
    /// Strict, verified capability classification.
    #[serde(default)]
    pub capability_status: CapabilityStatus,
    /// Resource requirements for execution.
    pub requirements: PluginRequirements,
    /// Expected SHA-256 hash of the package payload.
    #[serde(default)]
    pub sha256: Option<String>,
    /// Size of the plugin assets in bytes.
    #[serde(default)]
    pub size_bytes: u64,
    /// Host execution entrypoint or binary name.
    #[serde(default)]
    pub host_entrypoint: Option<String>,
    /// Worker execution entrypoint or mobile asset name.
    #[serde(default)]
    pub worker_entrypoint: Option<String>,
    /// Remote URL for downloading if not bundled.
    #[serde(default)]
    pub download_url: Option<String>,
}

impl PluginManifest {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
        kind: PluginKind,
        adapter_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            description: description.into(),
            author: "PocketBridge Community".to_string(),
            license: "Apache-2.0".to_string(),
            kind,
            adapter_id: adapter_id.into(),
            capability_status: CapabilityStatus::Experimental,
            requirements: PluginRequirements::default(),
            sha256: None,
            size_bytes: 0,
            host_entrypoint: None,
            worker_entrypoint: None,
            download_url: None,
        }
    }
}
