use std::collections::HashMap;
use std::sync::Arc;
use crate::error::PocketBridgeError;
use super::chunk_adapter::ChunkTransformAdapter;
use super::data_analytics::DataAnalyticsAdapter;
use super::image_batch::ImageBatchAdapter;
use super::traits::{AdapterExecutionResult, WorkloadAdapter};

/// Central registry managing all registered task adapters.
#[derive(Clone)]
pub struct AdapterRegistry {
    adapters: HashMap<&'static str, Arc<dyn WorkloadAdapter>>,
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(ChunkTransformAdapter));
        reg.register(Arc::new(ImageBatchAdapter));
        reg.register(Arc::new(DataAnalyticsAdapter));
        reg
    }
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    pub fn register(&mut self, adapter: Arc<dyn WorkloadAdapter>) {
        self.adapters.insert(adapter.adapter_id(), adapter);
    }

    pub fn get(&self, adapter_id: &str) -> Option<Arc<dyn WorkloadAdapter>> {
        self.adapters.get(adapter_id).cloned()
    }

    pub fn list_adapters(&self) -> Vec<(&'static str, &'static str)> {
        let mut list: Vec<_> = self.adapters.iter().map(|(&id, a)| (id, a.description())).collect();
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
