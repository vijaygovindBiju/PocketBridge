pub mod types;

use std::collections::HashMap;
use tokio::fs;
use crate::error::PocketBridgeError;
pub use types::DeviceProfile;

/// Collect host device profile from Arch Linux system files.
pub async fn collect_host_profile() -> Result<DeviceProfile, PocketBridgeError> {
    let cpu_info = fs::read_to_string("/proc/cpuinfo")
        .await
        .unwrap_or_default();
    let mem_info = fs::read_to_string("/proc/meminfo")
        .await
        .unwrap_or_default();
    let hostname = fs::read_to_string("/etc/hostname")
        .await
        .ok()
        .map(|s| s.trim().to_string());
    let kernel_version = fs::read_to_string("/proc/sys/kernel/osrelease")
        .await
        .unwrap_or_else(|_| "Linux".to_string())
        .trim()
        .to_string();

    let cpu_model = parse_cpu_model(&cpu_info);
    let cpu_cores = count_cpu_cores(&cpu_info);

    let ram_total = parse_meminfo_kb(&mem_info, "MemTotal").unwrap_or(0) * 1024;
    let ram_available = parse_meminfo_kb(&mem_info, "MemAvailable").map(|kb| kb * 1024);

    let gpu = detect_gpu().await;

    // Root filesystem storage stats via df
    let (storage_total, storage_available) = get_disk_stats("/").await;

    Ok(DeviceProfile {
        role: "host".into(),
        hostname,
        os: "Arch Linux".into(),
        os_version: kernel_version,
        architecture: std::env::consts::ARCH.to_string(),
        cpu_model,
        cpu_cores,
        ram_total_bytes: ram_total,
        ram_available_bytes: ram_available,
        gpu,
        storage_total_bytes: storage_total,
        storage_available_bytes: storage_available,
        android_api_level: None,
        device_model: None,
        soc: None,
        battery_percent: None,
        battery_charging: None,
        thermal_status: None,
    })
}

/// Parse CPU model name from /proc/cpuinfo
pub fn parse_cpu_model(cpu_info: &str) -> String {
    cpu_info
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string())
}

/// Count processors from /proc/cpuinfo
pub fn count_cpu_cores(cpu_info: &str) -> u32 {
    let count = cpu_info
        .lines()
        .filter(|l| l.starts_with("processor"))
        .count() as u32;
    if count == 0 {
        1
    } else {
        count
    }
}

/// Parse a field in KB from /proc/meminfo
pub fn parse_meminfo_kb(meminfo: &str, key: &str) -> Option<u64> {
    meminfo
        .lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
}

/// Detect GPU on Linux host
async fn detect_gpu() -> Option<String> {
    if let Ok(output) = tokio::process::Command::new("lspci").output().await {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains("VGA compatible controller") || line.contains("3D controller") {
                if let Some(pos) = line.find(':') {
                    if let Some(desc) = line[pos + 1..].split(':').nth(1) {
                        return Some(desc.trim().to_string());
                    }
                }
                return Some(line.trim().to_string());
            }
        }
    }
    None
}

/// Get disk stats in bytes using df
async fn get_disk_stats(path: &str) -> (Option<u64>, Option<u64>) {
    if let Ok(output) = tokio::process::Command::new("df")
        .args(["-B1", path])
        .output()
        .await
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = stdout.lines().nth(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let total = parts[1].parse::<u64>().ok();
                let avail = parts[3].parse::<u64>().ok();
                return (total, avail);
            }
        }
    }
    (None, None)
}

/// Build a worker DeviceProfile from ADB getprop properties as fallback/initial discovery
pub fn build_worker_profile_from_props(props: &HashMap<String, String>) -> DeviceProfile {
    let manufacturer = props
        .get("ro.product.manufacturer")
        .cloned()
        .unwrap_or_default();
    let model = props.get("ro.product.model").cloned().unwrap_or_default();
    let device_model = if !manufacturer.is_empty() && !model.is_empty() {
        Some(format!("{manufacturer} {model}"))
    } else if !model.is_empty() {
        Some(model)
    } else {
        None
    };

    let android_version = props
        .get("ro.build.version.release")
        .cloned()
        .unwrap_or_else(|| "Unknown".to_string());
    let api_level = props
        .get("ro.build.version.sdk")
        .and_then(|v| v.parse::<u32>().ok());

    let soc = props
        .get("ro.soc.model")
        .or_else(|| props.get("ro.board.platform"))
        .cloned();

    let arch = props
        .get("ro.product.cpu.abi")
        .cloned()
        .unwrap_or_else(|| "aarch64".to_string());

    DeviceProfile {
        role: "worker".into(),
        hostname: props.get("net.hostname").cloned(),
        os: "Android".into(),
        os_version: format!("Android {}", android_version),
        architecture: arch,
        cpu_model: soc.clone().unwrap_or_else(|| "ARM64".into()),
        cpu_cores: 8, // Typical mobile flagship; verified in app
        ram_total_bytes: 0,
        ram_available_bytes: None,
        gpu: None,
        storage_total_bytes: None,
        storage_available_bytes: None,
        android_api_level: api_level,
        device_model,
        soc,
        battery_percent: None,
        battery_charging: None,
        thermal_status: None,
    }
}
