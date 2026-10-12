use std::time::Instant;
use tokio::net::TcpStream;
use tracing::{debug, info};
use uuid::Uuid;
use crate::adapter::{ChunkTransformParams, TransformOutput, ADAPTER_ID};
use crate::error::PocketBridgeError;
use crate::protocol::{
    current_unix_ms, read_envelope, write_envelope, Envelope, JobAcceptedPayload, JobLimits,
    JobProgressPayload, JobRejectedPayload, JobRequirements, MessageKind, ResultManifestPayload,
    SubmitJobPayload,
};
use super::state::{JobExecutionResult, JobState};

pub struct RemoteJobClient;

impl RemoteJobClient {
    /// Submit and execute a chunk_transform job on the remote Android worker.
    pub async fn execute_remote_job<F>(
        stream: &mut TcpStream,
        params: &ChunkTransformParams,
        local_ref: Option<&TransformOutput>,
        mut progress: F,
    ) -> Result<JobExecutionResult, PocketBridgeError>
    where
        F: FnMut(f32, &str),
    {
        let job_id = Uuid::new_v4().to_string();
        info!("Submitting job {job_id} to worker");

        let t_total_start = Instant::now();

        // 1. Submit Job
        let submit_payload = SubmitJobPayload {
            job_id: job_id.clone(),
            adapter_id: ADAPTER_ID.to_string(),
            input_parameters: serde_json::to_value(params)?,
            requirements: JobRequirements {
                architecture: Some("aarch64".into()),
                min_memory_bytes: 16 * 1024 * 1024,
                max_wall_time_ms: 60_000,
            },
            limits: JobLimits {
                max_input_bytes: 50 * 1024 * 1024,
                max_output_bytes: 50 * 1024 * 1024,
                network_access: false,
            },
            deadline_unix_ms: current_unix_ms() + 60_000,
            idempotent: true,
            priority: 1,
        };

        let submit_env = Envelope::submit_job(submit_payload);
        let t_upload_start = Instant::now();
        write_envelope(stream, &submit_env).await?;
        let upload_duration_ms = t_upload_start.elapsed().as_secs_f64() * 1000.0;

        progress(0.05, "Job submitted. Awaiting worker acceptance...");

        // 2. Read acceptance / rejection
        let accept_env = read_envelope(stream).await?;
        match accept_env.kind {
            MessageKind::JobAccepted => {
                let accepted: JobAcceptedPayload = serde_json::from_value(accept_env.payload)?;
                debug!("Worker accepted job {} with attempt {}", accepted.job_id, accepted.attempt_id);
            }
            MessageKind::JobRejected => {
                let rejected: JobRejectedPayload = serde_json::from_value(accept_env.payload)?;
                return Ok(JobExecutionResult {
                    job_id,
                    adapter_id: ADAPTER_ID.to_string(),
                    state: JobState::Failed,
                    local_duration_ms: local_ref.map(|l| l.duration_ms),
                    remote_upload_ms: Some(upload_duration_ms),
                    remote_compute_ms: None,
                    remote_download_ms: None,
                    remote_total_ms: None,
                    speedup: None,
                    local_sha256: local_ref.map(|l| l.output_sha256.clone()),
                    remote_sha256: None,
                    outputs_match: false,
                    records_processed: 0,
                    error_message: Some(format!("Worker rejected: {} ({})", rejected.reason, rejected.code)),
                });
            }
            other => {
                return Err(PocketBridgeError::UnexpectedMessage {
                    expected: "JOB_ACCEPTED".into(),
                    got: format!("{:?}", other),
                });
            }
        }

        // 3. Receive progress and execution results
        let remote_compute_ms;
        let remote_sha256;
        let records_processed;
        let download_duration_ms;

        loop {
            let env = read_envelope(stream).await?;
            match env.kind {
                MessageKind::JobStarted => {
                    progress(0.10, "Worker started compute execution");
                }
                MessageKind::JobProgress => {
                    if let Ok(prog) = serde_json::from_value::<JobProgressPayload>(env.payload) {
                        let pct = (prog.progress_fraction * 100.0).clamp(0.0, 100.0);
                        progress(prog.progress_fraction, &format!("Worker computing... ({:.0}%)", pct));
                    }
                }
                MessageKind::ResultManifest => {
                    let t_dl_start = Instant::now();
                    let manifest: ResultManifestPayload = serde_json::from_value(env.payload)?;
                    download_duration_ms = t_dl_start.elapsed().as_secs_f64() * 1000.0;

                    remote_compute_ms = Some(manifest.compute_duration_ms as f64);
                    remote_sha256 = Some(manifest.output_sha256);
                    records_processed = manifest.records_processed;
                    progress(1.0, "Result received and verified");
                    break;
                }
                MessageKind::JobError => {
                    let err: crate::protocol::JobErrorPayload = serde_json::from_value(env.payload)?;
                    return Ok(JobExecutionResult {
                        job_id,
                        adapter_id: ADAPTER_ID.to_string(),
                        state: JobState::Failed,
                        local_duration_ms: local_ref.map(|l| l.duration_ms),
                        remote_upload_ms: Some(upload_duration_ms),
                        remote_compute_ms: None,
                        remote_download_ms: None,
                        remote_total_ms: None,
                        speedup: None,
                        local_sha256: local_ref.map(|l| l.output_sha256.clone()),
                        remote_sha256: None,
                        outputs_match: false,
                        records_processed: 0,
                        error_message: Some(format!("Worker error: {} ({})", err.message, err.error_code)),
                    });
                }
                MessageKind::Cancelled => {
                    return Ok(JobExecutionResult {
                        job_id,
                        adapter_id: ADAPTER_ID.to_string(),
                        state: JobState::Cancelled,
                        local_duration_ms: local_ref.map(|l| l.duration_ms),
                        remote_upload_ms: Some(upload_duration_ms),
                        remote_compute_ms: None,
                        remote_download_ms: None,
                        remote_total_ms: None,
                        speedup: None,
                        local_sha256: local_ref.map(|l| l.output_sha256.clone()),
                        remote_sha256: None,
                        outputs_match: false,
                        records_processed: 0,
                        error_message: Some("Job cancelled by worker".into()),
                    });
                }
                other => {
                    debug!("Ignoring message kind during execution: {:?}", other);
                }
            }
        }

        let remote_total_ms = t_total_start.elapsed().as_secs_f64() * 1000.0;

        let local_sha256 = local_ref.map(|l| l.output_sha256.clone());
        let outputs_match = match (&local_sha256, &remote_sha256) {
            (Some(l), Some(r)) => l == r,
            _ => true,
        };

        let mut res = JobExecutionResult {
            job_id,
            adapter_id: ADAPTER_ID.to_string(),
            state: if outputs_match { JobState::Succeeded } else { JobState::Failed },
            local_duration_ms: local_ref.map(|l| l.duration_ms),
            remote_upload_ms: Some(upload_duration_ms),
            remote_compute_ms,
            remote_download_ms: Some(download_duration_ms),
            remote_total_ms: Some(remote_total_ms),
            speedup: None,
            local_sha256,
            remote_sha256,
            outputs_match,
            records_processed,
            error_message: if !outputs_match {
                Some("Output SHA-256 mismatch between local and remote!".into())
            } else {
                None
            },
        };

        res.calculate_speedup();
        Ok(res)
    }

    /// Cancel a running job.
    pub async fn cancel_job(
        stream: &mut TcpStream,
        job_id: &str,
        reason: &str,
    ) -> Result<(), PocketBridgeError> {
        let env = Envelope::cancel_job(job_id, reason);
        write_envelope(stream, &env).await?;
        Ok(())
    }
}
