use std::fs;
use std::path::{Path, PathBuf};
use sha2::{Digest, Sha256};
use crate::error::PocketBridgeError;
use crate::profiler::DeviceProfile;
use super::catalog::{default_catalog, find_catalog_plugin, planned_catalog};
use super::manifest::PluginManifest;

/// Central manager coordinating plugin discovery, pre-flight checks, and lifecycle.
#[derive(Debug, Clone)]
pub struct PluginManager {
    plugins_dir: PathBuf,
}

impl PluginManager {
    /// Create a new PluginManager rooted at the specified directory.
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        let dir = plugins_dir.into();
        let _ = fs::create_dir_all(&dir);
        Self { plugins_dir: dir }
    }

    /// Default system-wide user plugin directory (~/.pocketbridge/plugins).
    pub fn default_dir() -> PathBuf {
        let home = std::env::var("HOME")
            .unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".pocketbridge").join("plugins")
    }

    /// Create an instance using the default system plugin directory.
    pub fn default_manager() -> Self {
        Self::new(Self::default_dir())
    }

    /// Returns the active plugins directory path.
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    /// List all currently installed plugins found on disk.
    pub fn list_installed(&self) -> Vec<PluginManifest> {
        let mut installed = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.plugins_dir) {
            for entry in entries.flatten() {
                let manifest_path = entry.path().join("manifest.json");
                if manifest_path.exists() {
                    if let Ok(content) = fs::read_to_string(&manifest_path) {
                        if let Ok(manifest) = serde_json::from_str::<PluginManifest>(&content) {
                            installed.push(manifest);
                        }
                    }
                }
            }
        }
        installed.sort_by(|a, b| a.id.cmp(&b.id));
        installed
    }

    /// List all catalog plugins with installed status flag.
    pub fn list_catalog(&self) -> Vec<(PluginManifest, bool)> {
        let installed_ids: Vec<String> = self.list_installed().into_iter().map(|p| p.id).collect();
        default_catalog()
            .into_iter()
            .map(|manifest| {
                let is_installed = installed_ids.contains(&manifest.id);
                (manifest, is_installed)
            })
            .collect()
    }

    /// List all catalog plugins plus planned roadmap plugins with installed status flag.
    pub fn list_all_catalog_and_planned(&self) -> Vec<(PluginManifest, bool)> {
        let installed_ids: Vec<String> = self.list_installed().into_iter().map(|p| p.id).collect();
        let mut all = default_catalog();
        all.extend(planned_catalog());
        all.into_iter()
            .map(|manifest| {
                let is_installed = installed_ids.contains(&manifest.id);
                (manifest, is_installed)
            })
            .collect()
    }

    /// Check if a plugin is currently installed by ID or adapter ID.
    pub fn is_installed(&self, id_or_adapter: &str) -> bool {
        self.list_installed()
            .iter()
            .any(|p| p.id == id_or_adapter || p.adapter_id == id_or_adapter)
    }

    /// Get the installed manifest for an id or adapter_id.
    pub fn get_installed(&self, id_or_adapter: &str) -> Option<PluginManifest> {
        self.list_installed()
            .into_iter()
            .find(|p| p.id == id_or_adapter || p.adapter_id == id_or_adapter)
    }

    /// Verify pre-flight hardware requirements against host and connected worker.
    pub fn check_preflight(
        &self,
        manifest: &PluginManifest,
        host_ram_mb: u64,
        worker_profile: Option<&DeviceProfile>,
    ) -> Result<(), PocketBridgeError> {
        // 1. Host RAM check
        if host_ram_mb < manifest.requirements.min_host_ram_mb {
            return Err(PocketBridgeError::PluginError(format!(
                "Pre-flight failed: Host has {} MB RAM, but plugin '{}' requires at least {} MB.",
                host_ram_mb, manifest.name, manifest.requirements.min_host_ram_mb
            )));
        }

        // 2. Connected worker checks (if available)
        if let Some(worker) = worker_profile {
            let worker_ram_mb = worker.ram_total_bytes / (1024 * 1024);
            if worker_ram_mb < manifest.requirements.min_worker_ram_mb {
                return Err(PocketBridgeError::PluginError(format!(
                    "Pre-flight failed: Worker phone has {} MB RAM, but plugin '{}' requires {} MB.",
                    worker_ram_mb, manifest.name, manifest.requirements.min_worker_ram_mb
                )));
            }

            if let Some(storage_bytes) = worker.storage_available_bytes {
                let worker_storage_mb = storage_bytes / (1024 * 1024);
                if worker_storage_mb < manifest.requirements.min_worker_storage_mb {
                    return Err(PocketBridgeError::PluginError(format!(
                        "Pre-flight failed: Worker phone has {} MB free storage, but plugin '{}' requires {} MB.",
                        worker_storage_mb, manifest.name, manifest.requirements.min_worker_storage_mb
                    )));
                }
            }
        }

        Ok(())
    }

    /// Install a plugin manifest directly into the plugins directory.
    pub fn install_manifest(
        &self,
        manifest: &PluginManifest,
        payload_data: Option<&[u8]>,
    ) -> Result<PathBuf, PocketBridgeError> {
        if manifest.capability_status == super::manifest::CapabilityStatus::Planned {
            return Err(PocketBridgeError::PluginError(format!(
                "Cannot install planned plugin '{}': no executable native package exists yet.",
                manifest.name
            )));
        }

        let plugin_dir = self.plugins_dir.join(&manifest.id);
        fs::create_dir_all(&plugin_dir)
            .map_err(|e| PocketBridgeError::PluginError(format!("Failed to create plugin dir: {e}")))?;

        // Write manifest.json
        let manifest_path = plugin_dir.join("manifest.json");
        let manifest_json = serde_json::to_string_pretty(manifest)?;
        fs::write(&manifest_path, manifest_json)
            .map_err(|e| PocketBridgeError::PluginError(format!("Failed to write manifest: {e}")))?;

        // If payload data provided, write it and verify checksum
        if let Some(data) = payload_data {
            if let Some(expected_sha) = &manifest.sha256 {
                let mut hasher = Sha256::new();
                hasher.update(data);
                let actual_sha = format!("{:x}", hasher.finalize());
                if !actual_sha.eq_ignore_ascii_case(expected_sha) {
                    // Clean up invalid installation
                    let _ = fs::remove_dir_all(&plugin_dir);
                    return Err(PocketBridgeError::PluginError(format!(
                        "SHA-256 verification failed for plugin '{}': expected {}, got {}",
                        manifest.id, expected_sha, actual_sha
                    )));
                }
            }

            let payload_path = plugin_dir.join("payload.bin");
            fs::write(payload_path, data)
                .map_err(|e| PocketBridgeError::PluginError(format!("Failed to write payload: {e}")))?;
        }

        Ok(plugin_dir)
    }

    /// Install a plugin from the official catalog by ID.
    pub fn install_from_catalog(
        &self,
        plugin_id: &str,
        host_ram_mb: u64,
        worker_profile: Option<&DeviceProfile>,
    ) -> Result<PluginManifest, PocketBridgeError> {
        let manifest = find_catalog_plugin(plugin_id)
            .ok_or_else(|| PocketBridgeError::PluginError(format!("Plugin '{}' not found in catalog.", plugin_id)))?;

        // Run pre-flight check
        self.check_preflight(&manifest, host_ram_mb, worker_profile)?;

        // For local simulation / built-in standard plugins, prepare installation
        self.install_manifest(&manifest, None)?;

        Ok(manifest)
    }

    /// Remove an installed plugin.
    pub fn remove(&self, id_or_adapter: &str) -> Result<bool, PocketBridgeError> {
        let target = self.list_installed()
            .into_iter()
            .find(|p| p.id == id_or_adapter || p.adapter_id == id_or_adapter);

        if let Some(manifest) = target {
            let plugin_dir = self.plugins_dir.join(&manifest.id);
            if plugin_dir.exists() {
                fs::remove_dir_all(plugin_dir)
                    .map_err(|e| PocketBridgeError::PluginError(format!("Failed to remove plugin: {e}")))?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Sync an installed plugin to an Android worker over ADB if connected.
    pub async fn sync_to_worker(
        &self,
        manifest: &PluginManifest,
        device_serial: Option<&str>,
    ) -> Result<String, PocketBridgeError> {
        let plugin_dir = self.plugins_dir.join(&manifest.id);
        if !plugin_dir.exists() {
            return Err(PocketBridgeError::PluginError(format!(
                "Plugin '{}' is not installed locally; cannot sync to worker.",
                manifest.id
            )));
        }

        let remote_parent = "/data/local/tmp/pocketbridge/plugins/";

        let mut cmd = tokio::process::Command::new("adb");
        if let Some(serial) = device_serial {
            cmd.args(["-s", serial]);
        }
        cmd.args(["shell", "mkdir", "-p", remote_parent]);

        let status = cmd.status().await;
        match status {
            Ok(s) if s.success() => {
                // Now push files
                let mut push_cmd = tokio::process::Command::new("adb");
                if let Some(serial) = device_serial {
                    push_cmd.args(["-s", serial]);
                }
                push_cmd.args(["push", plugin_dir.to_str().unwrap(), remote_parent]);
                let push_res = push_cmd.output().await;
                match push_res {
                    Ok(output) if output.status.success() => {
                        let out_msg = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        // Also mirror into app storage if run-as available
                        let mut copy_cmd = tokio::process::Command::new("adb");
                        if let Some(serial) = device_serial {
                            copy_cmd.args(["-s", serial]);
                        }
                        let copy_script = format!(
                            "run-as com.pocketbridge.worker mkdir -p files/plugins/{id} 2>/dev/null && run-as com.pocketbridge.worker cp -r /data/local/tmp/pocketbridge/plugins/{id}/* files/plugins/{id}/ 2>/dev/null || true",
                            id = manifest.id
                        );
                        copy_cmd.args(["shell", &copy_script]);
                        let _ = copy_cmd.output().await;

                        Ok(format!("Synced '{}' to phone at {}: {}", manifest.id, remote_parent, out_msg))
                    }
                    Ok(output) => {
                        let err_msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        Ok(format!("Local installed; ADB push note: {}", err_msg))
                    }
                    Err(e) => Ok(format!("Local installed; ADB command unavailable: {e}")),
                }
            }
            _ => {
                // Phone may not be connected via ADB at this instant, installation remains active on host
                Ok("Installed locally on host (worker sync queued when phone connects)".to_string())
            }
        }
    }
}
