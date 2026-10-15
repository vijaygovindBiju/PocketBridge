pub mod gguf;
pub mod partitioner;
pub mod activation;

pub use gguf::{GgufModel, GgufTensorInfo, GgmlType, GgufValue, GGUF_MAGIC};
pub use partitioner::{DistributedLayerPlan, LayerFootprint};
pub use activation::{ActivationFrame, DistributedTokenResult};

use std::path::{Path, PathBuf};
use crate::error::PocketBridgeError;

/// Automatically locate a model GGUF file from a path or Ollama model name.
pub fn resolve_model_path(query: &str) -> Result<PathBuf, PocketBridgeError> {
    let p = Path::new(query);
    if p.exists() && p.is_file() {
        return Ok(p.to_path_buf());
    }

    // Try finding in Ollama model store
    let candidate_dirs = [
        "/usr/share/ollama/.ollama/models",
        "/var/lib/ollama/.ollama/models",
        &format!("{}/.ollama/models", std::env::var("HOME").unwrap_or_default()),
    ];

    for base in &candidate_dirs {
        let manifest_path = if query.contains(':') {
            let parts: Vec<&str> = query.split(':').collect();
            PathBuf::from(base).join("manifests/registry.ollama.ai/library").join(parts[0]).join(parts[1])
        } else {
            PathBuf::from(base).join("manifests/registry.ollama.ai/library").join(query).join("latest")
        };

        if manifest_path.exists() {
            if let Ok(manifest_content) = std::fs::read_to_string(&manifest_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&manifest_content) {
                    if let Some(layers) = json.get("layers").and_then(|l| l.as_array()) {
                        for layer in layers {
                            if layer.get("mediaType").and_then(|m| m.as_str()) == Some("application/vnd.ollama.image.model") {
                                if let Some(digest) = layer.get("digest").and_then(|d| d.as_str()) {
                                    let clean_digest = digest.replace(':', "-");
                                    let blob_path = PathBuf::from(base).join("blobs").join(clean_digest);
                                    if blob_path.exists() {
                                        return Ok(blob_path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Err(PocketBridgeError::ModelError(format!(
        "Model '{}' not found. Please provide an absolute path to a .gguf file or an installed Ollama model tag (e.g., 'gemma2:27b').",
        query
    )))
}
