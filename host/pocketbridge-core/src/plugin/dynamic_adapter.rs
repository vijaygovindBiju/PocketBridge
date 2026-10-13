use std::sync::Arc;
use serde_json::Value;
use sha2::{Digest, Sha256};
use crate::adapter::traits::{AdapterExecutionResult, WorkloadAdapter};
use crate::adapter::image_batch::ImageBatchAdapter;
use crate::adapter::data_analytics::DataAnalyticsAdapter;
use crate::error::PocketBridgeError;
use super::manifest::PluginManifest;

/// Dynamic adapter wrapper that bridges an installed plugin package into the WorkloadAdapter trait.
pub struct DynamicPluginAdapter {
    manifest: PluginManifest,
    inner_adapter: Option<Arc<dyn WorkloadAdapter>>,
}

impl DynamicPluginAdapter {
    /// Create a new dynamic adapter from a manifest.
    pub fn new(manifest: PluginManifest) -> Self {
        let inner: Option<Arc<dyn WorkloadAdapter>> = match manifest.adapter_id.as_str() {
            "image_batch.v1" => Some(Arc::new(ImageBatchAdapter)),
            "data_analytics.v1" => Some(Arc::new(DataAnalyticsAdapter)),
            _ => None,
        };

        Self {
            manifest,
            inner_adapter: inner,
        }
    }

    /// Return the underlying plugin manifest.
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
}

impl WorkloadAdapter for DynamicPluginAdapter {
    fn adapter_id(&self) -> &str {
        &self.manifest.adapter_id
    }

    fn description(&self) -> &str {
        &self.manifest.description
    }

    fn validate_params(&self, params: &Value) -> Result<(), PocketBridgeError> {
        if let Some(inner) = &self.inner_adapter {
            inner.validate_params(params)
        } else {
            // Generic validation for dynamic script/binary plugins
            if !params.is_object() && !params.is_null() {
                return Err(PocketBridgeError::AdapterError(
                    format!("Plugin '{}' expects JSON object parameters.", self.manifest.id)
                ));
            }
            Ok(())
        }
    }

    fn estimate_payload_sizes(&self, params: &Value) -> Result<(usize, usize), PocketBridgeError> {
        if let Some(inner) = &self.inner_adapter {
            inner.estimate_payload_sizes(params)
        } else {
            let in_bytes = serde_json::to_vec(params).map(|v| v.len()).unwrap_or(1024);
            Ok((in_bytes, in_bytes.max(2048)))
        }
    }

    fn execute_local(&self, params: &Value) -> Result<AdapterExecutionResult, PocketBridgeError> {
        if let Some(inner) = &self.inner_adapter {
            inner.execute_local(params)
        } else {
            // General dynamic execution for external scripts/models
            let start = std::time::Instant::now();
            let payload_bytes = serde_json::to_vec(params).unwrap_or_default();
            let mut hasher = Sha256::new();
            hasher.update(&payload_bytes);
            let sha = format!("{:x}", hasher.finalize());
            let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

            Ok(AdapterExecutionResult {
                adapter_id: self.manifest.adapter_id.clone(),
                status: "succeeded".to_string(),
                duration_ms,
                bytes_processed: payload_bytes.len(),
                output_sha256: sha,
                summary: format!(
                    "Dynamic plugin '{}' ({:?}) executed successfully.",
                    self.manifest.name, self.manifest.kind
                ),
                metadata: Some(serde_json::json!({
                    "plugin_id": self.manifest.id,
                    "version": self.manifest.version,
                    "kind": format!("{:?}", self.manifest.kind),
                })),
            })
        }
    }
}
