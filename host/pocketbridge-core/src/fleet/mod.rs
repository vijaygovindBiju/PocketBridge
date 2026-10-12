pub mod health;
pub mod manager;
pub mod work_stealing;

use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

pub use health::FleetHealthMonitor;
pub use manager::{FleetManager, WorkerNode, WorkerNodeState};
pub use work_stealing::{ChunkResult, FleetExecutionSummary, FleetWorkQueue, WorkChunk};

use crate::error::PocketBridgeError;
use crate::job::{JobExecutionResult, JobState, RemoteJobClient};
use crate::protocol::{read_envelope, write_envelope, AgentRole, Envelope, MessageKind};

pub struct FleetCoordinator;

impl FleetCoordinator {
    /// Execute a distributed workload across a fleet of workers using dynamic work stealing and failover recovery.
    pub async fn execute_distributed<F>(
        fleet: &mut FleetManager,
        queue: Arc<Mutex<FleetWorkQueue>>,
        progress: F,
    ) -> Result<FleetExecutionSummary, PocketBridgeError>
    where
        F: FnMut(usize, usize, &str) + Send + 'static,
    {
        let active_nodes = fleet.active_nodes();
        if active_nodes.is_empty() {
            return Err(PocketBridgeError::FleetError(
                "No active workers available in fleet to execute workload".into(),
            ));
        }

        let total_initial_chunks = {
            let q = queue.lock().await;
            q.pending_count()
        };

        info!(
            "Starting distributed execution of {} chunks across {} worker nodes",
            total_initial_chunks,
            active_nodes.len()
        );

        let t_start = Instant::now();
        let progress_mutex = Arc::new(Mutex::new(progress));

        let mut handles = Vec::new();

        for node in fleet.active_nodes() {
            let serial = node.serial.clone();
            let port = node.port;
            let queue_clone = Arc::clone(&queue);
            let progress_clone = Arc::clone(&progress_mutex);

            let handle = tokio::spawn(async move {
                debug!("Worker thread started for {} (port {})", serial, port);

                loop {
                    // 1. Steal next chunk
                    let chunk = {
                        let mut q = queue_clone.lock().await;
                        if q.is_complete() {
                            break;
                        }
                        q.steal_next(&serial)
                    };

                    let chunk = match chunk {
                        Some(c) => c,
                        None => {
                            // Check if all chunks completed
                            let q = queue_clone.lock().await;
                            if q.is_complete() {
                                break;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                            continue;
                        }
                    };

                    // 2. Connect to worker
                    let mut stream = match TcpStream::connect(("127.0.0.1", port)).await {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to connect to worker {} on port {}: {e}", serial, port);
                            let mut q = queue_clone.lock().await;
                            q.fail_and_requeue(chunk.chunk_id);
                            break; // Node failure
                        }
                    };

                    // Handshake
                    let hello = Envelope::hello("pocketbridge-fleet", "0.1.0", AgentRole::Host);
                    if write_envelope(&mut stream, &hello).await.is_err() {
                        let mut q = queue_clone.lock().await;
                        q.fail_and_requeue(chunk.chunk_id);
                        break;
                    }
                    if let Ok(reply) = read_envelope(&mut stream).await {
                        if reply.kind != MessageKind::Hello {
                            let mut q = queue_clone.lock().await;
                            q.fail_and_requeue(chunk.chunk_id);
                            break;
                        }
                    } else {
                        let mut q = queue_clone.lock().await;
                        q.fail_and_requeue(chunk.chunk_id);
                        break;
                    }

                    // 3. Execute remote chunk
                    let exec_res: Result<JobExecutionResult, _> = RemoteJobClient::execute_generic_adapter_job(
                        &mut stream,
                        &chunk.adapter_id,
                        &chunk.parameters,
                        None,
                        None,
                        |_frac, _msg| {},
                    )
                    .await;

                    match exec_res {
                        Ok(res) if res.state == JobState::Succeeded => {
                            let sha = res.remote_sha256.unwrap_or_default();
                            let duration = res.remote_compute_ms.unwrap_or(res.remote_total_ms.unwrap_or(0.0));
                            let chunk_result = ChunkResult {
                                chunk_id: chunk.chunk_id,
                                worker_serial: serial.clone(),
                                output_sha256: sha,
                                duration_ms: duration,
                                records_processed: res.records_processed,
                            };

                            let mut q = queue_clone.lock().await;
                            q.complete_chunk(chunk.chunk_id, chunk_result);
                            let completed = q.completed_count();
                            let remaining = q.pending_count() + q.in_flight_count();

                            let mut prog = progress_clone.lock().await;
                            prog(completed, completed + remaining, &format!("Chunk {} completed by {}", chunk.chunk_id, serial));
                        }
                        Ok(_) | Err(_) => {
                            warn!("Chunk {} failed on worker {}. Requeueing for work-stealing...", chunk.chunk_id, serial);
                            let mut q = queue_clone.lock().await;
                            q.fail_and_requeue(chunk.chunk_id);
                        }
                    }
                }
            });

            handles.push(handle);
        }

        // Await all workers
        for h in handles {
            let _ = h.await;
        }

        let wall_clock_ms = t_start.elapsed().as_secs_f64() * 1000.0;
        let q = queue.lock().await;
        if !q.is_complete() {
            return Err(PocketBridgeError::FleetError(format!(
                "Fleet execution incomplete: only {} of {} chunks completed",
                q.completed_count(),
                total_initial_chunks
            )));
        }
        let summary = q.aggregate_results(wall_clock_ms);

        Ok(summary)
    }
}
