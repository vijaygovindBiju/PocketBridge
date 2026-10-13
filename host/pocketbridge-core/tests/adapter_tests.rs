use pocketbridge_core::{
    AdapterRegistry, ChunkTransformParams, DataAnalyticsParams, ImageBatchParams,
    ADAPTER_ID, DATA_ANALYTICS_ADAPTER_ID, IMAGE_BATCH_ADAPTER_ID,
};

#[test]
fn test_adapter_registry_discovery() {
    // 1. Out of the box (Option 2 micro-kernel): built-in hardware calibration adapter is immediately ready
    let default_reg = AdapterRegistry::default();
    assert!(default_reg.get(ADAPTER_ID).is_some());
    assert!(default_reg.list_adapters().iter().any(|(id, _)| *id == ADAPTER_ID));

    // 2. Full standard suite with decoupled image and analytics adapters
    let full_reg = AdapterRegistry::with_all_standard();
    let list = full_reg.list_adapters();
    assert!(list.len() >= 3);
    assert!(full_reg.get(ADAPTER_ID).is_some());
    assert!(full_reg.get(IMAGE_BATCH_ADAPTER_ID).is_some());
    assert!(full_reg.get(DATA_ANALYTICS_ADAPTER_ID).is_some());
}

#[test]
fn test_chunk_transform_adapter_execution() {
    let registry = AdapterRegistry::default();
    let params = serde_json::to_value(ChunkTransformParams {
        record_count: 5_000,
        rounds: 10,
        seed: 0x123456789,
    }).unwrap();

    let res = registry.execute_local(ADAPTER_ID, &params).unwrap();
    assert_eq!(res.status, "succeeded");
    assert_eq!(res.output_sha256.len(), 64);
    assert_eq!(res.bytes_processed, 5_000 * 8);
}

#[test]
fn test_image_batch_adapter_operations() {
    let registry = AdapterRegistry::with_all_standard();

    for op in ["grayscale_invert", "box_blur_3x3", "downscale_half"] {
        let params = serde_json::to_value(ImageBatchParams {
            image_count: 4,
            width: 64,
            height: 64,
            seed: 0xFEEDFACE,
            operation: op.to_string(),
        }).unwrap();

        let res = registry.execute_local(IMAGE_BATCH_ADAPTER_ID, &params).unwrap();
        assert_eq!(res.status, "succeeded");
        assert_eq!(res.output_sha256.len(), 64);
        assert!(res.bytes_processed > 0);
    }
}

#[test]
fn test_data_analytics_adapter_filtering() {
    let registry = AdapterRegistry::with_all_standard();
    let params = serde_json::to_value(DataAnalyticsParams {
        record_count: 10_000,
        seed: 0x987654321,
        filter_min: 0,
        filter_max: i64::MAX,
    }).unwrap();

    let res = registry.execute_local(DATA_ANALYTICS_ADAPTER_ID, &params).unwrap();
    assert_eq!(res.status, "succeeded");
    assert_eq!(res.output_sha256.len(), 64);
    assert!(res.bytes_processed > 0);
}

#[tokio::test]
async fn test_image_batch_remote_mock_lifecycle() {
    use pocketbridge_core::{
        read_envelope, write_envelope, Envelope, JobState, MessageKind, RemoteJobClient,
        ResultManifestPayload, SubmitJobPayload,
    };
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("local addr");

    let worker_handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("worker accepts connection");

        let submit_env = read_envelope(&mut socket).await.expect("read submit_job");
        assert_eq!(submit_env.kind, MessageKind::SubmitJob);
        let submit: SubmitJobPayload = serde_json::from_value(submit_env.payload).unwrap();
        assert_eq!(submit.adapter_id, IMAGE_BATCH_ADAPTER_ID);

        let accept_env = Envelope::job_accepted(&submit.job_id, "attempt-1", 1000);
        write_envelope(&mut socket, &accept_env).await.expect("send job_accepted");

        let started_env = Envelope::job_started(&submit.job_id, 1005);
        write_envelope(&mut socket, &started_env).await.expect("send job_started");

        let registry = AdapterRegistry::with_all_standard();
        let compute_out = registry.execute_local(IMAGE_BATCH_ADAPTER_ID, &submit.input_parameters).unwrap();

        let manifest_payload = ResultManifestPayload {
            job_id: submit.job_id,
            adapter_id: submit.adapter_id,
            status: "succeeded".to_string(),
            output_size_bytes: compute_out.bytes_processed,
            output_sha256: compute_out.output_sha256,
            compute_duration_ms: compute_out.duration_ms as u64,
            records_processed: 4,
            metadata: compute_out.metadata.unwrap_or(serde_json::Value::Null),
        };
        let manifest_env = Envelope::result_manifest(manifest_payload);
        write_envelope(&mut socket, &manifest_env).await.expect("send result_manifest");
    });

    let params = serde_json::to_value(ImageBatchParams {
        image_count: 4,
        width: 32,
        height: 32,
        seed: 0x9999,
        operation: "grayscale_invert".to_string(),
    }).unwrap();

    let registry = AdapterRegistry::with_all_standard();
    let local_res = registry.execute_local(IMAGE_BATCH_ADAPTER_ID, &params).unwrap();

    let mut client_stream = tokio::net::TcpStream::connect(addr).await.expect("connect to mock worker");
    let result = RemoteJobClient::execute_generic_adapter_job(
        &mut client_stream,
        IMAGE_BATCH_ADAPTER_ID,
        &params,
        Some(&local_res.output_sha256),
        Some(local_res.duration_ms),
        |_frac, _msg| {},
    ).await.expect("execute generic adapter job");

    assert_eq!(result.state, JobState::Succeeded);
    assert!(result.outputs_match);
    assert_eq!(result.remote_sha256.as_deref(), Some(local_res.output_sha256.as_str()));

    worker_handle.await.expect("worker task finishes cleanly");
}

#[tokio::test]
async fn test_data_analytics_remote_mock_lifecycle() {
    use pocketbridge_core::{
        read_envelope, write_envelope, Envelope, JobState, MessageKind, RemoteJobClient,
        ResultManifestPayload, SubmitJobPayload,
    };
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("local addr");

    let worker_handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("worker accepts connection");

        let submit_env = read_envelope(&mut socket).await.expect("read submit_job");
        assert_eq!(submit_env.kind, MessageKind::SubmitJob);
        let submit: SubmitJobPayload = serde_json::from_value(submit_env.payload).unwrap();
        assert_eq!(submit.adapter_id, DATA_ANALYTICS_ADAPTER_ID);

        let accept_env = Envelope::job_accepted(&submit.job_id, "attempt-1", 1000);
        write_envelope(&mut socket, &accept_env).await.expect("send job_accepted");

        let started_env = Envelope::job_started(&submit.job_id, 1005);
        write_envelope(&mut socket, &started_env).await.expect("send job_started");

        let registry = AdapterRegistry::with_all_standard();
        let compute_out = registry.execute_local(DATA_ANALYTICS_ADAPTER_ID, &submit.input_parameters).unwrap();

        let manifest_payload = ResultManifestPayload {
            job_id: submit.job_id,
            adapter_id: submit.adapter_id,
            status: "succeeded".to_string(),
            output_size_bytes: compute_out.bytes_processed,
            output_sha256: compute_out.output_sha256,
            compute_duration_ms: compute_out.duration_ms as u64,
            records_processed: 5_000,
            metadata: compute_out.metadata.unwrap_or(serde_json::Value::Null),
        };
        let manifest_env = Envelope::result_manifest(manifest_payload);
        write_envelope(&mut socket, &manifest_env).await.expect("send result_manifest");
    });

    let params = serde_json::to_value(DataAnalyticsParams {
        record_count: 5_000,
        seed: 0x5555,
        filter_min: 0,
        filter_max: i64::MAX,
    }).unwrap();

    let registry = AdapterRegistry::with_all_standard();
    let local_res = registry.execute_local(DATA_ANALYTICS_ADAPTER_ID, &params).unwrap();

    let mut client_stream = tokio::net::TcpStream::connect(addr).await.expect("connect to mock worker");
    let result = RemoteJobClient::execute_generic_adapter_job(
        &mut client_stream,
        DATA_ANALYTICS_ADAPTER_ID,
        &params,
        Some(&local_res.output_sha256),
        Some(local_res.duration_ms),
        |_frac, _msg| {},
    ).await.expect("execute generic adapter job");

    assert_eq!(result.state, JobState::Succeeded);
    assert!(result.outputs_match);
    assert_eq!(result.remote_sha256.as_deref(), Some(local_res.output_sha256.as_str()));

    worker_handle.await.expect("worker task finishes cleanly");
}
