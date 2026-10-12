pub mod reporter;
pub mod types;

use std::time::Instant;
use tokio::net::TcpStream;
use tracing::{debug, info};
use crate::error::PocketBridgeError;
use crate::protocol::{
    current_unix_ms, drain_raw_stream, read_envelope, write_envelope, write_raw_stream,
    BenchmarkEchoReplyPayload, Envelope, MessageKind,
};
pub use reporter::{format_bytes, format_summary_table, save_benchmark_report};
pub use types::{
    calculate_stats, BenchmarkConfig, BenchmarkReport, BenchmarkStats, PayloadSizeResult,
    SampleResult, TestType,
};

/// Run a single benchmark echo round-trip over an active TCP stream.
pub async fn run_single_echo(
    stream: &mut TcpStream,
    sequence: u32,
    binary_size: usize,
    priority: Option<u8>,
) -> Result<SampleResult, PocketBridgeError> {
    let t_start = Instant::now();
    let sent_at_ms = current_unix_ms();

    let mut env = Envelope::benchmark_echo(sequence, binary_size, sent_at_ms);
    if let Some(p) = priority {
        env = env.with_priority(p);
    }

    let (mut reader, mut writer) = stream.split();

    // 1. Send Envelope
    write_envelope(&mut writer, &env).await?;

    // 2. Concurrently write payload and read reply + echoed stream to prevent TCP buffer deadlocks on large (10MB+) streams
    let write_task = async {
        if binary_size > 0 {
            write_raw_stream(&mut writer, binary_size).await?;
        }
        Ok::<(), PocketBridgeError>(())
    };

    let read_task = async {
        let reply_env = read_envelope(&mut reader).await?;
        if reply_env.kind != MessageKind::BenchmarkEchoReply {
            return Err(PocketBridgeError::UnexpectedMessage {
                expected: "BENCHMARK_ECHO_REPLY".into(),
                got: format!("{:?}", reply_env.kind),
            });
        }
        if binary_size > 0 {
            drain_raw_stream(&mut reader, binary_size).await?;
        }
        Ok::<Envelope, PocketBridgeError>(reply_env)
    };

    let (_, reply_env) = tokio::try_join!(write_task, read_task)?;

    let round_trip_ms = t_start.elapsed().as_secs_f64() * 1000.0;

    let reply_payload: BenchmarkEchoReplyPayload =
        serde_json::from_value(reply_env.payload).map_err(PocketBridgeError::Json)?;

    let worker_processing_ms = reply_payload
        .worker_replied_at_unix_ms
        .saturating_sub(reply_payload.worker_received_at_unix_ms) as f64;

    debug!(
        "Echo seq={} size={}B rtt={:.2}ms worker={:.2}ms",
        sequence, binary_size, round_trip_ms, worker_processing_ms
    );

    Ok(SampleResult {
        sequence,
        round_trip_ms,
        worker_processing_ms,
        payload_size_bytes: binary_size,
    })
}

/// Run a complete benchmark suite according to the provided configuration.
pub async fn run_benchmark_suite<F>(
    stream: &mut TcpStream,
    config: &BenchmarkConfig,
    mut progress: F,
) -> Result<BenchmarkReport, PocketBridgeError>
where
    F: FnMut(usize, usize, &str),
{
    let mut results = Vec::new();
    let mut total_tests = 0;
    if config.include_latency_pings {
        total_tests += config.latency_ping_sizes.len() * config.runs_per_size as usize;
    }
    total_tests += config.payload_sizes.len() * config.runs_per_size as usize;

    let mut completed = 0;

    // 1. Latency Pings (small payloads: 64B, 1KB)
    if config.include_latency_pings {
        for &size in &config.latency_ping_sizes {
            let label = format!("Latency Ping ({})", format_bytes(size));
            progress(completed, total_tests, &label);

            let mut samples = Vec::new();
            for seq in 1..=config.runs_per_size {
                let sample = run_single_echo(stream, seq, size, config.priority).await?;
                samples.push(sample);
                completed += 1;
                progress(completed, total_tests, &label);
            }

            let stats = calculate_stats(&samples, size, false);
            results.push(PayloadSizeResult {
                payload_size_bytes: size,
                test_type: TestType::LatencyPing,
                runs: config.runs_per_size,
                stats,
                samples,
            });
        }
    }

    // 2. Throughput Tests (large payloads: 1MB, 10MB, 100MB)
    for &size in &config.payload_sizes {
        let label = format!("Throughput Echo ({})", format_bytes(size));
        progress(completed, total_tests, &label);

        let mut samples = Vec::new();
        for seq in 1..=config.runs_per_size {
            let sample = run_single_echo(stream, seq, size, config.priority).await?;
            samples.push(sample);
            completed += 1;
            progress(completed, total_tests, &label);
        }

        let stats = calculate_stats(&samples, size, true);
        results.push(PayloadSizeResult {
            payload_size_bytes: size,
            test_type: TestType::ThroughputEcho,
            runs: config.runs_per_size,
            stats,
            samples,
        });
    }

    let report = BenchmarkReport {
        timestamp: chrono::Local::now().to_rfc3339(),
        host_profile: None,
        worker_profile: None,
        results,
    };

    info!("Completed benchmark suite of {} test runs", completed);
    Ok(report)
}
