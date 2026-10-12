use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::info;
use crate::error::PocketBridgeError;
use super::types::{BenchmarkReport, PayloadSizeResult};

pub async fn save_benchmark_report(
    report: &BenchmarkReport,
    output_dir: impl AsRef<Path>,
) -> Result<PathBuf, PocketBridgeError> {
    let dir = output_dir.as_ref();
    if !dir.exists() {
        fs::create_dir_all(dir).await.map_err(|e| {
            PocketBridgeError::BenchmarkError(format!("Failed to create results dir: {e}"))
        })?;
    }

    let sanitized_ts = report.timestamp.replace([':', ' ', '-'], "_");
    let file_name = format!("benchmark_{sanitized_ts}.json");
    let file_path = dir.join(file_name);

    let json = serde_json::to_string_pretty(report).map_err(|e| {
        PocketBridgeError::BenchmarkError(format!("Failed to serialize report: {e}"))
    })?;

    fs::write(&file_path, json).await.map_err(|e| {
        PocketBridgeError::BenchmarkError(format!("Failed to write report file: {e}"))
    })?;

    info!("Saved benchmark report to {}", file_path.display());
    Ok(file_path)
}

pub fn format_summary_table(results: &[PayloadSizeResult]) -> String {
    let mut out = String::new();
    out.push_str("--------------------------------------------------------------------------------------------------\n");
    out.push_str(&format!(
        "{:<16} {:<16} {:<6} {:<10} {:<10} {:<10} {:<10} {:<12}\n",
        "Type", "Size", "Runs", "Median(ms)", "P95(ms)", "Min(ms)", "Max(ms)", "Throughput"
    ));
    out.push_str("--------------------------------------------------------------------------------------------------\n");

    for res in results {
        let size_str = format_bytes(res.payload_size_bytes);
        let throughput_str = match res.stats.throughput_mbps {
            Some(mbps) => format!("{:.2} Mbps", mbps),
            None => "-".to_string(),
        };

        out.push_str(&format!(
            "{:<16} {:<16} {:<6} {:<10.2} {:<10.2} {:<10.2} {:<10.2} {:<12}\n",
            res.test_type.to_string(),
            size_str,
            res.runs,
            res.stats.median_ms,
            res.stats.p95_ms,
            res.stats.min_ms,
            res.stats.max_ms,
            throughput_str
        ));
    }
    out.push_str("--------------------------------------------------------------------------------------------------\n");
    out
}

pub fn format_bytes(bytes: usize) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.2} GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{} B", bytes)
    }
}
