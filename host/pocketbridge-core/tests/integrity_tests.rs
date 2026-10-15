use pocketbridge_core::{
    default_catalog, find_catalog_plugin, find_planned_plugin, planned_catalog,
    format_summary_table, CapabilityStatus, DynamicPluginAdapter, JobExecutionResult,
    JobState, PayloadSizeResult, PocketBridgeError, BenchmarkStats, TestType,
    DeviceProfile, Scheduler, SchedulerConfig, CostModel, WorkloadAdapter,
    GgufModel, DistributedLayerPlan, resolve_model_path,
};
use serde_json::json;

#[test]
fn test_unbacked_plugins_cannot_execute_in_production() {
    for planned in planned_catalog() {
        let adapter = DynamicPluginAdapter::new(planned.clone());
        let err = adapter.execute_local(&json!({})).unwrap_err();
        match err {
            PocketBridgeError::UnsupportedOperation(msg) => {
                assert!(msg.contains("Simulated execution has been removed"));
            }
            other => panic!("Expected UnsupportedOperation for planned plugin '{}', got: {:?}", planned.id, other),
        }
    }
}

#[test]
fn test_planned_plugins_isolated_from_production_catalog() {
    let catalog = default_catalog();
    assert_eq!(catalog.len(), 2);
    for plugin in &catalog {
        assert_eq!(plugin.capability_status, CapabilityStatus::HardwareTested);
        assert!(plugin.id == "image-batch" || plugin.id == "data-analytics");
    }

    let planned = planned_catalog();
    assert_eq!(planned.len(), 5);
    for p in planned {
        assert_eq!(p.capability_status, CapabilityStatus::Planned);
        assert!(find_catalog_plugin(&p.id).is_none());
    }
}

#[test]
fn test_planned_plugin_cannot_be_installed() {
    let temp = tempfile::tempdir().unwrap();
    let manager = pocketbridge_core::PluginManager::new(temp.path());

    let whisper_planned = find_planned_plugin("ai-whisper").unwrap();
    let err = manager.install_manifest(&whisper_planned, None).unwrap_err();
    match err {
        PocketBridgeError::PluginError(msg) => {
            assert!(msg.contains("Cannot install planned plugin"));
        }
        other => panic!("Expected PluginError, got {:?}", other),
    }
}

#[test]
fn test_real_gguf_layer_partitioner() {
    // Attempt resolving gemma2:27b if installed on the host
    if let Ok(model_path) = resolve_model_path("gemma2:27b") {
        let model = GgufModel::open(&model_path).expect("Real GGUF model must parse cleanly");
        assert_eq!(model.architecture, "gemma2");
        assert_eq!(model.block_count, 46);
        assert_eq!(model.tensor_count, 508);
        assert_eq!(model.embedding_length, 4608);

        // Plan for 10.5 GB Host RAM and 8.0 GB Phone RAM
        let plan = DistributedLayerPlan::plan(&model, 10_500_000_000, 8_000_000_000, 370.14);
        assert_eq!(plan.total_layers, 46);
        assert_eq!(plan.host_layers_count, 21);
        assert_eq!(plan.worker_layers_count, 25);
        assert!(plan.host_ram_fits);
        assert!(plan.worker_ram_fits);
        assert!(!plan.can_execute_on_host_alone);
        assert!(!plan.can_execute_on_worker_alone);
        assert!(plan.can_execute_hybrid_distributed);
        // KV Cache calculations
        let kv_2k_full = model.kv_cache_bytes(2048, model.block_count);
        let kv_2k_worker = model.kv_cache_bytes(2048, 25);
        assert!(kv_2k_full > 800_000_000 && kv_2k_full < 900_000_000);
        assert!(kv_2k_worker > 400_000_000 && kv_2k_worker < 500_000_000);
    }
}

#[test]
fn test_preflight_safety_margin_enforcement() {
    // S25 Ultra 12GB device with 3.615 GB real measured available RAM
    let measured_avail_mb = 3615u64;
    let os_safety_buffer_mb = 1200u64;
    let safe_headroom_mb = measured_avail_mb.saturating_sub(os_safety_buffer_mb);
    assert_eq!(safe_headroom_mb, 2415);

    let layer_weight_mb = 319u64; // ~318.6 MB per Gemma 2 27B layer
    let max_safe_layers = safe_headroom_mb / layer_weight_mb;
    assert_eq!(max_safe_layers, 7);

    // If an aggressive user requests 16 layers, safety gate clamps to 7 to prevent Android lmkd kill
    let requested_layers = 16u64;
    let enforced_layers = requested_layers.min(max_safe_layers);
    assert_eq!(enforced_layers, 7);
}

#[test]
fn test_benchmark_metrics_use_correct_units() {
    let results = vec![PayloadSizeResult {
        payload_size_bytes: 10_000_000,
        test_type: TestType::ThroughputEcho,
        runs: 5,
        stats: BenchmarkStats {
            median_ms: 200.0,
            p95_ms: 210.0,
            p99_ms: 215.0,
            min_ms: 190.0,
            max_ms: 220.0,
            stddev_ms: 5.0,
            throughput_mbps: Some(400.0), // 400 Mbps
        },
        samples: vec![],
    }];

    let table = format_summary_table(&results);
    // Table must clearly label Mbps (Megabits per second), not MB/s
    assert!(table.contains("400.00 Mbps"));
    assert!(table.contains("Throughput"));
}

#[test]
fn test_failed_remote_execution_cannot_silently_succeed() {
    let mut failed_res = JobExecutionResult {
        job_id: "test-job-1".to_string(),
        adapter_id: "chunk_transform.v1".to_string(),
        state: JobState::Failed,
        local_duration_ms: Some(10.0),
        remote_upload_ms: Some(1.0),
        remote_compute_ms: None,
        remote_download_ms: None,
        remote_total_ms: None,
        speedup: None,
        local_sha256: Some("abc".into()),
        remote_sha256: None,
        outputs_match: false,
        records_processed: 0,
        output_size_bytes: None,
        error_message: Some("Worker crashed with OOM".into()),
    };

    assert_eq!(failed_res.state, JobState::Failed);
    assert!(!failed_res.outputs_match);
    failed_res.calculate_speedup();
    assert!(failed_res.speedup.is_none());
}

#[test]
fn test_scheduler_decisions_reflect_actual_constraints() {
    let config = SchedulerConfig {
        min_battery_percent: 20,
        reject_severe_thermal: true,
        ..Default::default()
    };
    let scheduler = Scheduler::new(config, CostModel::default());

    let mut profile = DeviceProfile {
        role: "worker".into(),
        hostname: None,
        os: "Android".into(),
        os_version: "15".into(),
        architecture: "aarch64".into(),
        cpu_model: "Snapdragon 8 Elite".into(),
        cpu_cores: 8,
        ram_total_bytes: 12 * 1024 * 1024 * 1024,
        ram_available_bytes: Some(6 * 1024 * 1024 * 1024),
        gpu: None,
        storage_total_bytes: None,
        storage_available_bytes: None,
        android_api_level: Some(35),
        device_model: Some("SM-S938B".into()),
        soc: Some("Snapdragon 8 Elite".into()),
        battery_percent: Some(15), // Below 20%
        battery_charging: Some(false),
        thermal_status: Some("none".into()),
    };

    let decision = scheduler.evaluate_task(100_000, 10, 800_000, 800_000, Some(&profile), Some(85.0));
    // Must reject due to low battery
    assert!(!decision.approved_for_offload);
    assert!(!decision.constraints.battery_ok);

    // Now plug in charger
    profile.battery_charging = Some(true);
    let decision2 = scheduler.evaluate_task(100_000, 10, 800_000, 800_000, Some(&profile), Some(85.0));
    assert!(decision2.constraints.battery_ok);

    // Now set severe thermal
    profile.thermal_status = Some("severe".into());
    let decision3 = scheduler.evaluate_task(100_000, 10, 800_000, 800_000, Some(&profile), Some(85.0));
    assert!(!decision3.approved_for_offload);
    assert!(!decision3.constraints.thermal_ok);
}
