pub mod catalog;
pub mod dynamic_adapter;
pub mod manifest;
pub mod manager;

pub use catalog::{default_catalog, find_catalog_plugin};
pub use dynamic_adapter::DynamicPluginAdapter;
pub use manifest::{PluginKind, PluginManifest, PluginRequirements};
pub use manager::PluginManager;
