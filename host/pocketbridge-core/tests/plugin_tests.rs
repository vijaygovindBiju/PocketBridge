use pocketbridge_core::{
    default_catalog, find_catalog_plugin, AdapterRegistry, DeviceProfile,
    PluginKind, PluginManifest, PluginManager, PluginRequirements,
};
use sha2::{Digest, Sha256};
use tempfile::tempdir;

#[test]
fn test_plugin_catalog_discovery() {
    let catalog = default_catalog();
    assert_eq!(catalog.len(), 2);

    // Verify presence of verified native plugins
    let image_plugin = find_catalog_plugin("image-batch");
    assert!(image_plugin.is_some());
    assert_eq!(image_plugin.as_ref().unwrap().adapter_id, "image_batch.v1");
    assert_eq!(image_plugin.unwrap().capability_status, pocketbridge_core::CapabilityStatus::HardwareTested);

    let analytics_plugin = find_catalog_plugin("data-analytics");
    assert!(analytics_plugin.is_some());
    assert_eq!(analytics_plugin.as_ref().unwrap().adapter_id, "data_analytics.v1");
    assert_eq!(analytics_plugin.unwrap().capability_status, pocketbridge_core::CapabilityStatus::HardwareTested);

    // Unbacked plugins are removed from production catalog
    assert!(find_catalog_plugin("ai-whisper").is_none());
    assert!(find_catalog_plugin("llm-llama").is_none());

    // But discoverable in planned roadmap catalog
    let whisper_planned = pocketbridge_core::find_planned_plugin("ai-whisper");
    assert!(whisper_planned.is_some());
    assert_eq!(whisper_planned.as_ref().unwrap().kind, PluginKind::AiModel);
    assert_eq!(whisper_planned.unwrap().capability_status, pocketbridge_core::CapabilityStatus::Planned);
}

#[test]
fn test_plugin_preflight_ram_and_storage_gates() {
    let temp = tempdir().unwrap();
    let manager = PluginManager::new(temp.path());

    let manifest = PluginManifest {
        id: "heavy-ai-model".to_string(),
        name: "Heavy AI Model".to_string(),
        version: "1.0.0".to_string(),
        description: "Requires 16GB RAM".to_string(),
        author: "AI Lab".to_string(),
        license: "Apache-2.0".to_string(),
        kind: PluginKind::AiModel,
        adapter_id: "heavy_ai.v1".to_string(),
        capability_status: pocketbridge_core::CapabilityStatus::Experimental,
        requirements: PluginRequirements {
            min_host_ram_mb: 8192,
            min_worker_ram_mb: 4096,
            min_worker_storage_mb: 500,
        },
        sha256: None,
        size_bytes: 2_000_000_000,
        host_entrypoint: None,
        worker_entrypoint: None,
        download_url: None,
    };

    let mut low_worker = DeviceProfile {
        role: "worker".to_string(),
        hostname: None,
        os: "Android".to_string(),
        os_version: "15".to_string(),
        architecture: "aarch64".to_string(),
        cpu_model: "Snapdragon 8 Elite".to_string(),
        cpu_cores: 8,
        ram_total_bytes: 2 * 1024 * 1024 * 1024, // 2 GB
        ram_available_bytes: Some(1024 * 1024 * 1024),
        gpu: None,
        storage_total_bytes: None,
        storage_available_bytes: Some(100 * 1024 * 1024), // 100 MB
        android_api_level: Some(35),
        device_model: Some("SM-S938B".to_string()),
        soc: Some("SM8750".to_string()),
        battery_percent: Some(95),
        battery_charging: Some(true),
        thermal_status: Some("None".to_string()),
    };

    // 1. Host has too little RAM
    let res_host = manager.check_preflight(&manifest, 4096, Some(&low_worker));
    assert!(res_host.is_err());
    assert!(res_host.unwrap_err().to_string().contains("Host has 4096 MB RAM"));

    // 2. Worker has too little RAM
    let res_worker_ram = manager.check_preflight(&manifest, 16384, Some(&low_worker));
    assert!(res_worker_ram.is_err());
    assert!(res_worker_ram.unwrap_err().to_string().contains("Worker phone has 2048 MB RAM"));

    // 3. Worker has enough RAM but too little storage
    low_worker.ram_total_bytes = 12 * 1024 * 1024 * 1024; // 12 GB
    let res_worker_storage = manager.check_preflight(&manifest, 16384, Some(&low_worker));
    assert!(res_worker_storage.is_err());
    assert!(res_worker_storage.unwrap_err().to_string().contains("free storage"));

    // 4. Sufficient resources passes
    low_worker.storage_available_bytes = Some(10 * 1024 * 1024 * 1024); // 10 GB
    let res_pass = manager.check_preflight(&manifest, 16384, Some(&low_worker));
    assert!(res_pass.is_ok());
}

#[test]
fn test_plugin_install_lifecycle_and_dynamic_registry_execution() {
    let temp = tempdir().unwrap();
    let manager = PluginManager::new(temp.path());

    // Initially nothing installed
    assert!(manager.list_installed().is_empty());
    assert!(!manager.is_installed("image-batch"));

    // Install image-batch from catalog into our test directory
    let installed = manager.install_from_catalog("image-batch", 16384, None).unwrap();
    assert_eq!(installed.id, "image-batch");
    assert!(manager.is_installed("image-batch"));
    assert!(manager.is_installed("image_batch.v1"));

    // Verify AdapterRegistry with manager can execute it dynamically
    let mut registry = AdapterRegistry::default(); // has chunk_transform.v1 built-in
    assert!(registry.get("chunk_transform.v1").is_some());
    
    registry.load_from_manager(&manager);
    assert!(registry.get("image_batch.v1").is_some());

    // Execute dynamic image_batch workload
    let params = serde_json::json!({
        "image_count": 2,
        "width": 32,
        "height": 32,
        "seed": 12345,
        "operation": "grayscale_invert"
    });
    let res = registry.execute_local("image_batch.v1", &params).unwrap();
    assert_eq!(res.status, "succeeded");
    assert_eq!(res.output_sha256.len(), 64);

    // Remove the plugin
    let removed = manager.remove("image-batch").unwrap();
    assert!(removed);
    assert!(!manager.is_installed("image-batch"));
}

#[test]
fn test_plugin_checksum_verification() {
    let temp = tempdir().unwrap();
    let manager = PluginManager::new(temp.path());

    let payload = b"Hello PocketBridge Plugin Payload";
    let mut hasher = Sha256::new();
    hasher.update(payload);
    let valid_sha = format!("{:x}", hasher.finalize());

    let mut manifest = PluginManifest::new(
        "custom-test-plugin",
        "Test Plugin",
        "0.1.0",
        "Custom test plugin",
        PluginKind::Script,
        "custom_test.v1",
    );
    manifest.sha256 = Some(valid_sha);

    // 1. Valid payload succeeds
    let path = manager.install_manifest(&manifest, Some(payload)).unwrap();
    assert!(path.exists());
    assert!(path.join("manifest.json").exists());
    assert!(path.join("payload.bin").exists());

    // 2. Corrupted payload is rejected and directory cleaned up
    manifest.id = "corrupted-test-plugin".to_string();
    let corrupted_payload = b"Corrupted bytes!";
    let err = manager.install_manifest(&manifest, Some(corrupted_payload));
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("SHA-256 verification failed"));
    assert!(!temp.path().join("corrupted-test-plugin").exists());
}
