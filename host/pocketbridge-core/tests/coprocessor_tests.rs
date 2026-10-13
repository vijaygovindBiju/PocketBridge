use pocketbridge_core::{
    partition_workload, read_envelope, write_envelope, AdapterRegistry, ChunkTransformParams,
    CoProcessor, DataAnalyticsParams, Envelope, ImageBatchParams, MessageKind,
    ResultManifestPayload, SubmitJobPayload, ADAPTER_ID, DATA_ANALYTICS_ADAPTER_ID,
    IMAGE_BATCH_ADAPTER_ID,
};
use tokio::net::TcpListener;

#[test]
fn test_chunk_transform_partitioning() {
    let params = serde_json::to_value(ChunkTransformParams {
        record_count: 100_000,
        rounds: 20,
        seed: 0x12345678,
    })
    .unwrap();

    let (host_val, worker_val) = partition_workload(ADAPTER_ID, &params, 0.6).unwrap();

    let host_p: ChunkTransformParams = serde_json::from_value(host_val).unwrap();
    let worker_p: ChunkTransformParams = serde_json::from_value(worker_val).unwrap();

    assert_eq!(host_p.record_count, 60_000);
    assert_eq!(worker_p.record_count, 40_000);
    assert_eq!(host_p.record_count + worker_p.record_count, 100_000);
    assert_eq!(worker_p.seed, 0x12345678 + 60_000);
}

#[test]
fn test_image_batch_partitioning() {
    let params = serde_json::to_value(ImageBatchParams {
        image_count: 50,
        width: 128,
        height: 128,
        seed: 0xABCD,
        operation: "grayscale_invert".to_string(),
    })
    .unwrap();

    let (host_val, worker_val) = partition_workload(IMAGE_BATCH_ADAPTER_ID, &params, 0.4).unwrap();

    let host_p: ImageBatchParams = serde_json::from_value(host_val).unwrap();
    let worker_p: ImageBatchParams = serde_json::from_value(worker_val).unwrap();

    assert_eq!(host_p.image_count, 20);
    assert_eq!(worker_p.image_count, 30);
    assert_eq!(host_p.image_count + worker_p.image_count, 50);
}

#[test]
fn test_data_analytics_partitioning() {
    let params = serde_json::to_value(DataAnalyticsParams {
        record_count: 500_000,
        seed: 0xCAFE,
        filter_min: 0,
        filter_max: i64::MAX,
    })
    .unwrap();

    let (host_val, worker_val) = partition_workload(DATA_ANALYTICS_ADAPTER_ID, &params, 0.7).unwrap();

    let host_p: DataAnalyticsParams = serde_json::from_value(host_val).unwrap();
    let worker_p: DataAnalyticsParams = serde_json::from_value(worker_val).unwrap();

    assert_eq!(host_p.record_count, 350_000);
    assert_eq!(worker_p.record_count, 150_000);
    assert_eq!(host_p.record_count + worker_p.record_count, 500_000);
}

#[tokio::test]
async fn test_hybrid_coprocessor_mock_execution() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("local addr");

    let worker_handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");

        let submit_env = read_envelope(&mut socket).await.expect("read submit");
        assert_eq!(submit_env.kind, MessageKind::SubmitJob);
        let submit: SubmitJobPayload = serde_json::from_value(submit_env.payload).unwrap();

        let accept_env = Envelope::job_accepted(&submit.job_id, "att-1", 100);
        write_envelope(&mut socket, &accept_env).await.expect("send accept");

        let started_env = Envelope::job_started(&submit.job_id, 105);
        write_envelope(&mut socket, &started_env).await.expect("send started");

        let registry = AdapterRegistry::default();
        let compute_out = registry.execute_local(&submit.adapter_id, &submit.input_parameters).unwrap();

        let manifest = ResultManifestPayload {
            job_id: submit.job_id,
            adapter_id: submit.adapter_id,
            status: "succeeded".to_string(),
            output_size_bytes: compute_out.bytes_processed,
            output_sha256: compute_out.output_sha256,
            compute_duration_ms: compute_out.duration_ms as u64,
            records_processed: 5_000,
            metadata: compute_out.metadata.unwrap_or(serde_json::Value::Null),
        };
        write_envelope(&mut socket, &Envelope::result_manifest(manifest)).await.expect("send manifest");
    });

    let mut stream = tokio::net::TcpStream::connect(addr).await.expect("connect");

    let params = serde_json::to_value(ChunkTransformParams {
        record_count: 10_000,
        rounds: 10,
        seed: 0x1234,
    })
    .unwrap();

    let result = CoProcessor::execute_hybrid(
        &mut stream,
        ADAPTER_ID,
        &params,
        0.5,
        |_f, _m| {},
    )
    .await
    .expect("hybrid execution succeeds");

    assert_eq!(result.total_records, 10_000);
    assert_eq!(result.host_records, 5_000);
    assert_eq!(result.worker_records, 5_000);
    assert_eq!(result.combined_sha256.len(), 64);
    assert!(result.effective_speedup > 0.0);
    assert!(result.throughput_records_sec > 0.0);

    worker_handle.await.expect("worker finished");
}

#[test]
fn test_dynamic_partition_allocation_profiles() {
    use pocketbridge_core::{DeviceProfile, PartitionConfig};

    let host_profile = DeviceProfile {
        role: "host".into(),
        hostname: None,
        os: "Linux".into(),
        os_version: "6.12.0".into(),
        architecture: "x86_64".into(),
        cpu_model: "13th Gen Intel Core i5-13420H".into(),
        cpu_cores: 12,
        ram_total_bytes: 16 * 1024 * 1024 * 1024,
        ram_available_bytes: Some(10 * 1024 * 1024 * 1024),
        gpu: None,
        storage_total_bytes: None,
        storage_available_bytes: None,
        android_api_level: None,
        device_model: None,
        soc: None,
        battery_percent: None,
        battery_charging: None,
        thermal_status: None,
    };

    // 1. High-End Flagship (Snapdragon 8 Elite / S25 Ultra)
    let s25_ultra = DeviceProfile {
        role: "worker".into(),
        hostname: None,
        os: "Android".into(),
        os_version: "15".into(),
        architecture: "aarch64".into(),
        cpu_model: "Snapdragon 8 Elite".into(),
        cpu_cores: 8,
        ram_total_bytes: 12 * 1024 * 1024 * 1024,
        ram_available_bytes: Some(6 * 1024 * 1024 * 1024),
        gpu: Some("Adreno 830".into()),
        storage_total_bytes: None,
        storage_available_bytes: None,
        android_api_level: Some(35),
        device_model: Some("SM-S938B".into()),
        soc: Some("SM8750".into()),
        battery_percent: Some(80),
        battery_charging: Some(true),
        thermal_status: Some("none".into()),
    };

    let (cfg_s25, rationale_s25) = PartitionConfig::from_device_profiles(&host_profile, &s25_ultra, 1.0);
    // On idle host, S25 Ultra gets ~50% of the workload
    assert!(cfg_s25.host_ratio >= 0.45 && cfg_s25.host_ratio <= 0.55);
    assert!(rationale_s25.contains("Dynamic partition calibrated"));

    // 2. Budget Phone (Helio / low-end SoC, 4 cores @ 2.0 GHz)
    let budget_phone = DeviceProfile {
        role: "worker".into(),
        hostname: None,
        os: "Android".into(),
        os_version: "11".into(),
        architecture: "aarch64".into(),
        cpu_model: "Helio G35".into(),
        cpu_cores: 4,
        ram_total_bytes: 3 * 1024 * 1024 * 1024,
        ram_available_bytes: Some(500 * 1024 * 1024),
        gpu: None,
        storage_total_bytes: None,
        storage_available_bytes: None,
        android_api_level: Some(30),
        device_model: Some("Generic Budget".into()),
        soc: Some("MT6765".into()),
        battery_percent: Some(50),
        battery_charging: Some(false),
        thermal_status: Some("none".into()),
    };

    let (cfg_budget, _) = PartitionConfig::from_device_profiles(&host_profile, &budget_phone, 1.0);
    // Budget phone gets much less work (~20% or less, host handles ~78%)
    assert!(cfg_budget.host_ratio > 0.70);

    // 3. Low Battery Guard (10% battery not charging -> auto throttle to 95% host)
    let low_battery_phone = DeviceProfile {
        battery_percent: Some(10),
        battery_charging: Some(false),
        ..s25_ultra.clone()
    };
    let (cfg_low_bat, _) = PartitionConfig::from_device_profiles(&host_profile, &low_battery_phone, 1.0);
    assert!(cfg_low_bat.host_ratio >= 0.90);
}
