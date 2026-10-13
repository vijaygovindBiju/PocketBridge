use anyhow::{Context, Result};
use clap::Subcommand;
use pocketbridge_core::{
    collect_host_profile, find_catalog_plugin, AdbTransport, DeviceProfile,
    PluginManager, ADAPTER_ID, DEFAULT_PORT,
};

#[derive(Subcommand, Debug)]
pub enum PluginCommands {
    /// List installed plugins, built-in engines, and catalog plugins
    List,
    /// Install a plugin from the community catalog or local file
    Install {
        /// Plugin identifier (e.g., 'image-batch', 'data-analytics', 'ai-whisper', 'browser-cdp', 'media-ffmpeg')
        id: String,
        /// Port for worker ADB query and sync (default: 9876)
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
    },
    /// Remove an installed plugin
    Remove {
        /// Plugin identifier or adapter ID to remove
        id: String,
    },
    /// View detailed information and requirements for a plugin
    Info {
        /// Plugin identifier or adapter ID
        id: String,
    },
}

pub async fn execute(cmd: PluginCommands) -> Result<()> {
    let manager = PluginManager::default_manager();

    match cmd {
        PluginCommands::List => {
            println!("PocketBridge Plugin & Workload Hub");
            println!("==========================================================================");
            println!("Plugins Directory: {}\n", manager.plugins_dir().display());

            println!("[Built-in Core Hardware Engine] (Zero-download offline link verification)");
            println!("  * {:<16} {:<10} {}", ADAPTER_ID, "[BUILT-IN]", "SIMD Micro-Kernel Hardware Calibration & Benchmark");
            println!();

            let installed = manager.list_installed();
            println!("[Installed Plugins] ({})", installed.len());
            if installed.is_empty() {
                println!("  (No external plugins installed yet. Install one using: pocketbridge plugin install <id>)");
            } else {
                for p in &installed {
                    let size_mb = p.size_bytes as f64 / (1024.0 * 1024.0);
                    println!(
                        "  * {:<16} v{:<7} [{:?}] {:.1} MB - {}",
                        p.id, p.version, p.kind, size_mb, p.name
                    );
                }
            }
            println!();

            println!("[Available Community & Official Catalog]");
            let catalog = manager.list_catalog();
            for (p, is_installed) in catalog {
                let status_str = if is_installed { "[INSTALLED]" } else { "[AVAILABLE]" };
                let size_mb = p.size_bytes as f64 / (1024.0 * 1024.0);
                println!(
                    "  {:<12} {:<16} v{:<7} ({:.1} MB) - {}",
                    status_str, p.id, p.version, size_mb, p.name
                );
                println!("               Description: {}", p.description);
                println!("               Min RAM: Host {} MB / Worker {} MB", p.requirements.min_host_ram_mb, p.requirements.min_worker_ram_mb);
            }
            println!("--------------------------------------------------------------------------");
            println!("To install:  pocketbridge plugin install <id>");
            println!("To inspect:  pocketbridge plugin info <id>");
            println!("To remove:   pocketbridge plugin remove <id>");
        }

        PluginCommands::Install { id, port } => {
            let plugins_to_install = if id.eq_ignore_ascii_case("all") {
                pocketbridge_core::default_catalog()
            } else {
                let manifest = find_catalog_plugin(&id)
                    .with_context(|| format!("Plugin '{}' not found in catalog. Run 'pocketbridge plugin list' to view available plugins.", id))?;
                vec![manifest]
            };

            // 1. Host profile
            let host_profile = match collect_host_profile().await {
                Ok(p) => p,
                Err(_) => DeviceProfile {
                    role: "host".to_string(),
                    hostname: None,
                    os: "Linux".to_string(),
                    os_version: "6.x".to_string(),
                    architecture: "x86_64".to_string(),
                    cpu_model: "Intel CPU".to_string(),
                    cpu_cores: 8,
                    ram_total_bytes: 16 * 1024 * 1024 * 1024,
                    ram_available_bytes: None,
                    gpu: None,
                    storage_total_bytes: None,
                    storage_available_bytes: None,
                    android_api_level: None,
                    device_model: None,
                    soc: None,
                    battery_percent: None,
                    battery_charging: None,
                    thermal_status: None,
                },
            };
            let host_ram_mb = host_profile.ram_total_bytes / (1024 * 1024);

            // 2. Query worker profile over ADB if available
            let transport = AdbTransport::new(port);
            let worker_profile: Option<DeviceProfile> = if transport.check_device().await.is_ok() {
                let mut props = transport.query_getprop().await.unwrap_or_default();
                if let Some(mem_bytes) = transport.query_mem_total_bytes().await {
                    props.insert("PB_MEM_TOTAL_BYTES".to_string(), mem_bytes.to_string());
                }
                if let Some(storage_bytes) = transport.query_storage_available_bytes().await {
                    props.insert("PB_STORAGE_AVAIL_BYTES".to_string(), storage_bytes.to_string());
                }
                Some(pocketbridge_core::profiler::build_worker_profile_from_props(&props))
            } else {
                None
            };

            for manifest in plugins_to_install {
                println!("\nInstalling Plugin: '{}' ({})", manifest.id, manifest.name);
                println!("==========================================================================");
                println!(">> [1/4] Host resources: {} MB RAM available", host_ram_mb);

                if let Some(worker) = &worker_profile {
                    let w_ram = worker.ram_total_bytes / (1024 * 1024);
                    println!(">> [2/4] Worker phone: {} ({} MB RAM)", worker.device_model.as_deref().unwrap_or("Android"), w_ram);
                } else {
                    println!(">> [2/4] Worker phone not currently connected; skipping worker pre-flight.");
                }

                // 3. Pre-flight check
                print!(">> [3/4] Running pre-flight hardware requirement gates... ");
                manager.check_preflight(&manifest, host_ram_mb, worker_profile.as_ref())
                    .context("Pre-flight hardware check failed")?;
                println!("[OK]");

                // 4. Install locally
                print!(">> [4/4] Installing plugin package to host... ");
                let dir = manager.install_manifest(&manifest, None)?;
                println!("[OK] Installed at {}", dir.display());

                // 5. Worker sync over ADB
                let sync_note = manager.sync_to_worker(&manifest, None).await?;
                println!(">> [Sync] {}", sync_note);

                println!("--------------------------------------------------------------------------");
                println!("Plugin '{}' (v{}) successfully installed!", manifest.name, manifest.version);
                println!("Adapter '{}' is now active. Run: pocketbridge run -a {}", manifest.adapter_id, manifest.adapter_id);
            }
        }

        PluginCommands::Remove { id } => {
            println!("Removing Plugin: '{}'...", id);
            let removed = manager.remove(&id)?;
            if removed {
                println!("Plugin '{}' removed successfully.", id);
            } else {
                println!("Plugin '{}' was not installed.", id);
            }
        }

        PluginCommands::Info { id } => {
            let manifest = manager.get_installed(&id)
                .or_else(|| find_catalog_plugin(&id))
                .with_context(|| format!("Plugin '{}' not found in installed plugins or catalog.", id))?;

            let is_installed = manager.is_installed(&manifest.id);
            let size_mb = manifest.size_bytes as f64 / (1024.0 * 1024.0);

            println!("Plugin Details: {}", manifest.name);
            println!("==========================================================================");
            println!("ID:           {}", manifest.id);
            println!("Version:      {}", manifest.version);
            println!("Status:       {}", if is_installed { "Installed" } else { "Not Installed" });
            println!("Engine Kind:  {:?}", manifest.kind);
            println!("Adapter ID:   {}", manifest.adapter_id);
            println!("Author:       {}", manifest.author);
            println!("License:      {}", manifest.license);
            println!("Size:         {:.2} MB ({} bytes)", size_mb, manifest.size_bytes);
            if let Some(sha) = &manifest.sha256 {
                println!("SHA-256:      {}", sha);
            }
            if let Some(url) = &manifest.download_url {
                println!("Download URL: {}", url);
            }
            println!("Description:  {}", manifest.description);
            println!("--------------------------------------------------------------------------");
            println!("Requirements:");
            println!("  * Min Host RAM:       {} MB", manifest.requirements.min_host_ram_mb);
            println!("  * Min Worker RAM:     {} MB", manifest.requirements.min_worker_ram_mb);
            println!("  * Min Worker Storage: {} MB", manifest.requirements.min_worker_storage_mb);
        }
    }

    Ok(())
}
