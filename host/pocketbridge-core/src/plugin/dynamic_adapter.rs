use std::sync::Arc;
use serde_json::Value;
use crate::adapter::traits::{AdapterExecutionResult, WorkloadAdapter};
use crate::adapter::image_batch::ImageBatchAdapter;
use crate::adapter::data_analytics::DataAnalyticsAdapter;
use crate::error::PocketBridgeError;
use super::manifest::PluginManifest;

/// Dynamic adapter wrapper that bridges an installed plugin package into the WorkloadAdapter trait.
/// Only plugins with genuine native execution backends are permitted to execute.
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

    /// Check if this plugin has a real, working native backend.
    pub fn is_executable(&self) -> bool {
        self.inner_adapter.is_some()
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
            Err(PocketBridgeError::UnsupportedOperation(format!(
                "Plugin '{}' (adapter '{}') has no executable production backend installed. Execution rejected.",
                self.manifest.id, self.manifest.adapter_id
            )))
        }
    }

    fn estimate_payload_sizes(&self, params: &Value) -> Result<(usize, usize), PocketBridgeError> {
        if let Some(inner) = &self.inner_adapter {
            inner.estimate_payload_sizes(params)
        } else {
            Err(PocketBridgeError::UnsupportedOperation(format!(
                "Plugin '{}' has no executable backend to estimate payload sizes.",
                self.manifest.id
            )))
        }
    }

    fn execute_local(&self, params: &Value) -> Result<AdapterExecutionResult, PocketBridgeError> {
        if let Some(inner) = &self.inner_adapter {
            inner.execute_local(params)
        } else {
            Err(PocketBridgeError::UnsupportedOperation(format!(
                "Cannot execute plugin '{}' ({:?}): no genuine native execution backend is installed on this host. Simulated execution has been removed from production paths.",
                self.manifest.id, self.manifest.capability_status
            )))
        }
    }
}
