use std::collections::HashMap;
use std::sync::Arc;
use crate::error::PocketBridgeError;
use super::chunk_adapter::ChunkTransformAdapter;
use super::data_analytics::DataAnalyticsAdapter;
use super::image_batch::ImageBatchAdapter;
use super::traits::{AdapterExecutionResult, WorkloadAdapter};
use crate::plugin::dynamic_adapter::DynamicPluginAdapter;
use crate::plugin::manager::PluginManager;

/// Central registry managing all registered task adapters.
#[derive(Clone)]
pub struct AdapterRegistry {
    adapters: HashMap<String, Arc<dyn WorkloadAdapter>>,
}

impl Default for AdapterRegistry {
    /// Option 2 (Micro-Kernel with Built-in Calibration Benchmark):
    /// Initializes with `chunk_transform.v1` as the built-in hardware calibration engine
    /// (allowing users to immediately benchmark/verify their USB link without network downloads),
    /// and dynamically registers any plugins found in ~/.pocketbridge/plugins/.
    fn default() -> Self {
        let mut reg = Self::new();
        // 1. Built-in Core Engine (Zero-download hardware calibration benchmark)
        reg.register(Arc::new(ChunkTransformAdapter));

        // 2. Load installed external plugins
        reg.load_installed_plugins();
        reg
    }
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Full standard suite (chunk_transform + image_batch + data_analytics + installed plugins).
    pub fn with_all_standard() -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(ChunkTransformAdapter));
        reg.register(Arc::new(ImageBatchAdapter));
        reg.register(Arc::new(DataAnalyticsAdapter));
        reg.load_installed_plugins();
        reg
    }

    /// Discover and register installed plugins from ~/.pocketbridge/plugins/
    pub fn load_installed_plugins(&mut self) {
        let manager = PluginManager::default_manager();
        self.load_from_manager(&manager);
    }

    /// Load plugins from a custom PluginManager.
    pub fn load_from_manager(&mut self, manager: &PluginManager) {
        for manifest in manager.list_installed() {
            let dynamic = Arc::new(DynamicPluginAdapter::new(manifest));
            if dynamic.is_executable() {
                self.register(dynamic);
            }
        }
    }

    pub fn register(&mut self, adapter: Arc<dyn WorkloadAdapter>) {
        self.adapters.insert(adapter.adapter_id().to_string(), adapter);
    }

    pub fn get(&self, adapter_id: &str) -> Option<Arc<dyn WorkloadAdapter>> {
        self.adapters.get(adapter_id).cloned()
    }

    pub fn list_adapters(&self) -> Vec<(&str, &str)> {
        let mut list: Vec<_> = self.adapters.values().map(|a| (a.adapter_id(), a.description())).collect();
        list.sort_by_key(|(id, _)| *id);
        list
    }

    pub fn execute_local(&self, adapter_id: &str, params: &serde_json::Value) -> Result<AdapterExecutionResult, PocketBridgeError> {
        let adapter = self.get(adapter_id)
            .ok_or_else(|| PocketBridgeError::AdapterError(format!("Adapter not found: {adapter_id}")))?;
        adapter.validate_params(params)?;
        adapter.execute_local(params)
    }
}
