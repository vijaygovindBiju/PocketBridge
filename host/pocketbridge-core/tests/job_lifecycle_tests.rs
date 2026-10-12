use pocketbridge_core::{
    read_envelope, write_envelope, ChunkTransformParams, Envelope, JobState,
    LocalExecutor, MessageKind, RemoteJobClient, ResultManifestPayload, SubmitJobPayload,
};
use tokio::net::TcpListener;

#[test]
fn test_local_chunk_transform_deterministic() {
    let params = ChunkTransformParams {
        record_count: 10_000,
        rounds: 10,
        seed: 0xDEADBEEFCAFEBABE,
    };

    let out1 = LocalExecutor::run_transform(&params);
    let out2 = LocalExecutor::run_transform(&params);

    assert_eq!(out1.output_sha256, out2.output_sha256);
    assert_eq!(out1.records_processed, 10_000);
    assert_eq!(out1.output_size_bytes, 80_000);
    assert!(!out1.output_sha256.is_empty());
}

#[tokio::test]
async fn test_mock_worker_job_lifecycle() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("local addr");

    // Spawn mock Android worker task
    let worker_handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("worker accepts connection");

        // 1. Read SUBMIT_JOB
        let submit_env = read_envelope(&mut socket).await.expect("read submit_job");
        assert_eq!(submit_env.kind, MessageKind::SubmitJob);
        let submit: SubmitJobPayload = serde_json::from_value(submit_env.payload).unwrap();
        assert_eq!(submit.adapter_id, "chunk_transform.v1");

        // 2. Send JOB_ACCEPTED
        let accept_env = Envelope::job_accepted(&submit.job_id, "attempt-1", 1000);
        write_envelope(&mut socket, &accept_env).await.expect("send job_accepted");

        // 3. Send JOB_STARTED
        let started_env = Envelope::job_started(&submit.job_id, 1005);
        write_envelope(&mut socket, &started_env).await.expect("send job_started");

        // 4. Send JOB_PROGRESS
        let prog_env = Envelope::job_progress(pocketbridge_core::JobProgressPayload {
            job_id: submit.job_id.clone(),
            progress_fraction: 0.5,
            memory_used_bytes: Some(10 * 1024 * 1024),
            elapsed_compute_ms: 15,
        });
        write_envelope(&mut socket, &prog_env).await.expect("send job_progress");

        // 5. Worker computes transform using same math
        let params: ChunkTransformParams = serde_json::from_value(submit.input_parameters).unwrap();
        let compute_out = LocalExecutor::run_transform(&params);

        // 6. Send RESULT_MANIFEST
        let manifest_env = Envelope::result_manifest(ResultManifestPayload {
            job_id: submit.job_id,
            adapter_id: submit.adapter_id,
            status: "succeeded".into(),
            output_size_bytes: compute_out.output_size_bytes,
            output_sha256: compute_out.output_sha256,
            compute_duration_ms: compute_out.duration_ms as u64,
            records_processed: compute_out.records_processed,
            metadata: serde_json::json!({}),
        });
        write_envelope(&mut socket, &manifest_env).await.expect("send result_manifest");
    });

    // Client side:
    let mut client_stream = tokio::net::TcpStream::connect(addr).await.expect("client connects");
    let params = ChunkTransformParams {
        record_count: 5_000,
        rounds: 5,
        seed: 0x9988776655443322,
    };

    // Run local reference
    let local_ref = LocalExecutor::run_transform(&params);

    // Execute remote job through RemoteJobClient
    let mut progress_count = 0;
    let res = RemoteJobClient::execute_remote_job(
        &mut client_stream,
        &params,
        Some(&local_ref),
        |_fraction, _msg| {
            progress_count += 1;
        },
    ).await.expect("execute remote job");

    assert_eq!(res.state, JobState::Succeeded);
    assert!(res.outputs_match);
    assert_eq!(res.local_sha256, res.remote_sha256);
    assert!(res.speedup.is_some());
    assert!(progress_count >= 2);

    worker_handle.await.expect("mock worker completes");
}
