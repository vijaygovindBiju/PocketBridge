pub mod catalog;
pub mod dynamic_adapter;
pub mod manifest;
pub mod manager;

pub use catalog::{default_catalog, find_catalog_plugin, find_planned_plugin, planned_catalog};
pub use dynamic_adapter::DynamicPluginAdapter;
pub use manifest::{CapabilityStatus, PluginKind, PluginManifest, PluginRequirements};
pub use manager::PluginManager;
