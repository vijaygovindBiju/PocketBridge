use std::sync::Arc;
use pocketbridge_core::{
    read_envelope, write_envelope, AdapterRegistry, ChunkTransformParams, Envelope,
    FleetCoordinator, FleetManager, FleetWorkQueue, MessageKind, ResultManifestPayload,
    SubmitJobPayload, WorkChunk, ADAPTER_ID,
};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_fleet_work_queue_stealing_and_requeue() {
    let mut queue = FleetWorkQueue::new();

    for i in 0..10 {
        queue.push_chunk(WorkChunk {
            chunk_id: i,
            adapter_id: ADAPTER_ID.to_string(),
            parameters: serde_json::json!({ "record_count": 1000 }),
            records_count: 1000,
        });
    }

    assert_eq!(queue.pending_count(), 10);

    // Worker A steals 2 chunks
    let c0 = queue.steal_next("worker_a").unwrap();
    let c1 = queue.steal_next("worker_a").unwrap();
    assert_eq!(c0.chunk_id, 0);
    assert_eq!(c1.chunk_id, 1);
    assert_eq!(queue.in_flight_count(), 2);
    assert_eq!(queue.pending_count(), 8);

    // Worker A unplugs/fails -> failover requeues both chunks
    let requeued = queue.fail_worker_chunks("worker_a");
    assert_eq!(requeued, 2);
    assert_eq!(queue.in_flight_count(), 0);
    assert_eq!(queue.pending_count(), 10);

    // Worker B steals and completes all 10 chunks
    for _ in 0..10 {
        let chunk = queue.steal_next("worker_b").unwrap();
        queue.complete_chunk(
            chunk.chunk_id,
            pocketbridge_core::ChunkResult {
                chunk_id: chunk.chunk_id,
                worker_serial: "worker_b".to_string(),
                output_sha256: "deadbeef".to_string(),
                duration_ms: 5.0,
                records_processed: chunk.records_count,
            },
        );
    }

    assert!(queue.is_complete());
    assert_eq!(queue.completed_count(), 10);
}

#[tokio::test]
async fn test_fleet_coordinator_distributed_execution() {
    let listener_1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port_1 = listener_1.local_addr().unwrap().port();

    let listener_2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port_2 = listener_2.local_addr().unwrap().port();

    // Spawn 2 mock worker servers
    let server = |listener: TcpListener| async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                while let Ok(env) = read_envelope(&mut socket).await {
                    match env.kind {
                        MessageKind::Hello => {
                            let reply = Envelope::hello("mock-worker", "0.1.0", pocketbridge_core::AgentRole::Worker);
                            let _ = write_envelope(&mut socket, &reply).await;
                        }
                        MessageKind::SubmitJob => {
                            let submit: SubmitJobPayload = serde_json::from_value(env.payload).unwrap();
                            let accept = Envelope::job_accepted(&submit.job_id, "att-1", 100);
                            let _ = write_envelope(&mut socket, &accept).await;
                            let started = Envelope::job_started(&submit.job_id, 105);
                            let _ = write_envelope(&mut socket, &started).await;

                            let registry = AdapterRegistry::default();
                            let out = registry.execute_local(&submit.adapter_id, &submit.input_parameters).unwrap();

                            let manifest = ResultManifestPayload {
                                job_id: submit.job_id,
                                adapter_id: submit.adapter_id,
                                status: "succeeded".to_string(),
                                output_size_bytes: out.bytes_processed,
                                output_sha256: out.output_sha256,
                                compute_duration_ms: out.duration_ms as u64,
                                records_processed: 2_000,
                                metadata: serde_json::Value::Null,
                            };
                            let _ = write_envelope(&mut socket, &Envelope::result_manifest(manifest)).await;
                        }
                        _ => {}
                    }
                }
            });
        }
    };

    tokio::spawn(server(listener_1));
    tokio::spawn(server(listener_2));

    let mut fleet = FleetManager::new(9876);
    fleet.add_node("phone_s25_ultra_1".to_string(), port_1);
    fleet.add_node("phone_s25_ultra_2".to_string(), port_2);

    let queue = Arc::new(Mutex::new(FleetWorkQueue::new()));

    // Enqueue 6 chunks
    {
        let mut q = queue.lock().await;
        for i in 0..6 {
            let p = ChunkTransformParams {
                record_count: 2_000,
                rounds: 10,
                seed: 0x1000 + (i as u64) * 2000,
            };
            q.push_chunk(WorkChunk {
                chunk_id: i,
                adapter_id: ADAPTER_ID.to_string(),
                parameters: serde_json::to_value(p).unwrap(),
                records_count: 2_000,
            });
        }
    }

    let summary = FleetCoordinator::execute_distributed(
        &mut fleet,
        queue,
        |_done, _total, _msg| {},
    )
    .await
    .expect("fleet execution succeeds");

    assert_eq!(summary.total_chunks, 6);
    assert_eq!(summary.total_records, 12_000);
    assert_eq!(summary.combined_sha256.len(), 64);
    assert!(summary.worker_chunk_counts.len() >= 1);
}

#[tokio::test]
async fn test_fleet_failover_hot_unplug() {
    let listener_1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port_1 = listener_1.local_addr().unwrap().port();

    let listener_2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port_2 = listener_2.local_addr().unwrap().port();

    // Worker 1 server only processes 1 chunk, then terminates (simulating hot unplug)
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener_1.accept().await {
            while let Ok(env) = read_envelope(&mut socket).await {
                match env.kind {
                    MessageKind::Hello => {
                        let reply = Envelope::hello("mock-worker-1", "0.1.0", pocketbridge_core::AgentRole::Worker);
                        let _ = write_envelope(&mut socket, &reply).await;
                    }
                    MessageKind::SubmitJob => {
                        let submit: SubmitJobPayload = serde_json::from_value(env.payload).unwrap();
                        let accept = Envelope::job_accepted(&submit.job_id, "att-1", 100);
                        let _ = write_envelope(&mut socket, &accept).await;
                        let started = Envelope::job_started(&submit.job_id, 105);
                        let _ = write_envelope(&mut socket, &started).await;

                        let registry = AdapterRegistry::default();
                        let out = registry.execute_local(&submit.adapter_id, &submit.input_parameters).unwrap();

                        let manifest = ResultManifestPayload {
                            job_id: submit.job_id,
                            adapter_id: submit.adapter_id,
                            status: "succeeded".to_string(),
                            output_size_bytes: out.bytes_processed,
                            output_sha256: out.output_sha256,
                            compute_duration_ms: out.duration_ms as u64,
                            records_processed: 1_000,
                            metadata: serde_json::Value::Null,
                        };
                        let _ = write_envelope(&mut socket, &Envelope::result_manifest(manifest)).await;
                        // Drop socket and terminate listener immediately after 1 chunk
                        drop(socket);
                        drop(listener_1);
                        break;
                    }
                    _ => {}
                }
            }
        }
    });

    // Worker 2 server is reliable and processes all incoming jobs
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener_2.accept().await {
            tokio::spawn(async move {
                while let Ok(env) = read_envelope(&mut socket).await {
                    match env.kind {
                        MessageKind::Hello => {
                            let reply = Envelope::hello("mock-worker-2", "0.1.0", pocketbridge_core::AgentRole::Worker);
                            let _ = write_envelope(&mut socket, &reply).await;
                        }
                        MessageKind::SubmitJob => {
                            let submit: SubmitJobPayload = serde_json::from_value(env.payload).unwrap();
                            let accept = Envelope::job_accepted(&submit.job_id, "att-2", 100);
                            let _ = write_envelope(&mut socket, &accept).await;
                            let started = Envelope::job_started(&submit.job_id, 105);
                            let _ = write_envelope(&mut socket, &started).await;

                            let registry = AdapterRegistry::default();
                            let out = registry.execute_local(&submit.adapter_id, &submit.input_parameters).unwrap();

                            let manifest = ResultManifestPayload {
                                job_id: submit.job_id,
                                adapter_id: submit.adapter_id,
                                status: "succeeded".to_string(),
                                output_size_bytes: out.bytes_processed,
                                output_sha256: out.output_sha256,
                                compute_duration_ms: out.duration_ms as u64,
                                records_processed: 1_000,
                                metadata: serde_json::Value::Null,
                            };
                            let _ = write_envelope(&mut socket, &Envelope::result_manifest(manifest)).await;
                        }
                        _ => {}
                    }
                }
            });
        }
    });

    let mut fleet = FleetManager::new(9876);
    fleet.add_node("worker_node_1".to_string(), port_1);
    fleet.add_node("worker_node_2".to_string(), port_2);

    let queue = Arc::new(Mutex::new(FleetWorkQueue::new()));

    // Enqueue 4 chunks
    {
        let mut q = queue.lock().await;
        for i in 0..4 {
            let p = ChunkTransformParams {
                record_count: 1_000,
                rounds: 5,
                seed: 0x2000 + (i as u64) * 1000,
            };
            q.push_chunk(WorkChunk {
                chunk_id: i,
                adapter_id: ADAPTER_ID.to_string(),
                parameters: serde_json::to_value(p).unwrap(),
                records_count: 1_000,
            });
        }
    }

    let summary = FleetCoordinator::execute_distributed(
        &mut fleet,
        queue,
        |_done, _total, _msg| {},
    )
    .await
    .expect("fleet execution succeeds even after worker 1 unplugging");

    assert_eq!(summary.total_chunks, 4);
    assert_eq!(summary.total_records, 4_000);
    assert_eq!(summary.worker_chunk_counts.get("worker_node_1"), Some(&1));
    assert_eq!(summary.worker_chunk_counts.get("worker_node_2"), Some(&3));
}
