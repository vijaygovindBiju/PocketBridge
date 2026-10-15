use std::collections::HashMap;
use tokio::net::TcpStream;
use tokio::process::Command;
use tracing::{debug, info, warn};
use crate::error::PocketBridgeError;

pub const DEFAULT_PORT: u16 = 9876;

#[derive(Debug, Clone)]
pub struct AdbTransport {
    port: u16,
}

impl Default for AdbTransport {
    fn default() -> Self {
        Self { port: DEFAULT_PORT }
    }
}

impl AdbTransport {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Check if ADB is installed and return list of attached devices.
    pub async fn list_devices(&self) -> Result<Vec<String>, PocketBridgeError> {
        let output = Command::new("adb")
            .args(["devices", "-l"])
            .output()
            .await
            .map_err(|e| PocketBridgeError::AdbError(format!("Failed to execute adb: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PocketBridgeError::AdbError(format!("adb devices failed: {stderr}")));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut devices = Vec::new();
        for line in stdout.lines().skip(1) {
            let trimmed = line.trim();
            if !trimmed.is_empty() && trimmed.contains("device") && !trimmed.starts_with('*') {
                devices.push(trimmed.to_string());
            }
        }
        Ok(devices)
    }

    /// Check if at least one authorized device is connected.
    pub async fn check_device(&self) -> Result<String, PocketBridgeError> {
        let devices = self.list_devices().await?;
        if devices.is_empty() {
            return Err(PocketBridgeError::AdbError(
                "No connected ADB devices found. Ensure USB debugging is enabled on your phone."
                    .into(),
            ));
        }
        info!("Found ADB device: {}", devices[0]);
        Ok(devices[0].clone())
    }

    /// Run `adb forward tcp:<port> tcp:<port>`
    pub async fn setup_forwarding(&self) -> Result<(), PocketBridgeError> {
        debug!("Configuring ADB port forwarding on port {}", self.port);
        let output = Command::new("adb")
            .args([
                "forward",
                &format!("tcp:{}", self.port),
                &format!("tcp:{}", self.port),
            ])
            .output()
            .await
            .map_err(|e| PocketBridgeError::AdbError(format!("Failed to execute adb forward: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PocketBridgeError::AdbError(format!(
                "adb forward failed: {stderr}"
            )));
        }
        info!("ADB forwarding established: 127.0.0.1:{} -> device:{}", self.port, self.port);
        Ok(())
    }

    /// Remove ADB port forwarding.
    pub async fn remove_forwarding(&self) -> Result<(), PocketBridgeError> {
        let output = Command::new("adb")
            .args(["forward", "--remove", &format!("tcp:{}", self.port)])
            .output()
            .await
            .map_err(|e| PocketBridgeError::AdbError(format!("Failed to remove adb forward: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("adb forward --remove warning: {stderr}");
        }
        Ok(())
    }

    /// Connect to the forwarded TCP port on localhost.
    pub async fn connect_tcp(&self) -> Result<TcpStream, PocketBridgeError> {
        let addr = format!("127.0.0.1:{}", self.port);
        debug!("Connecting to worker at {}", addr);
        TcpStream::connect(&addr).await.map_err(|e| {
            PocketBridgeError::ConnectionFailed(format!(
                "Failed to connect to worker at {addr}: {e}. Is the worker service running?"
            ))
        })
    }

    /// Query device properties via `adb shell getprop` as supplementary profiler info.
    pub async fn query_getprop(&self) -> Result<HashMap<String, String>, PocketBridgeError> {
        let output = Command::new("adb")
            .args(["shell", "getprop"])
            .output()
            .await
            .map_err(|e| PocketBridgeError::AdbError(format!("adb shell getprop failed: {e}")))?;

        let mut props = HashMap::new();
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                // Format: [key]: [value]
                if let Some((k_part, v_part)) = line.split_once("]: [") {
                    if let Some(key) = k_part.strip_prefix('[') {
                        let val = v_part.strip_suffix(']').unwrap_or(v_part);
                        props.insert(key.to_string(), val.to_string());
                    }
                }
            }
        }
        Ok(props)
    }

    /// Query total RAM in bytes from /proc/meminfo over ADB
    pub async fn query_mem_total_bytes(&self) -> Option<u64> {
        let output = Command::new("adb")
            .args(["shell", "cat /proc/meminfo"])
            .output()
            .await
            .ok()?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.starts_with("MemTotal:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<u64>() {
                            return Some(kb * 1024);
                        }
                    }
                }
            }
        }
        None
    }

    /// Query available storage in bytes from df /data over ADB
    pub async fn query_storage_available_bytes(&self) -> Option<u64> {
        let output = Command::new("adb")
            .args(["shell", "df -k /data"])
            .output()
            .await
            .ok()?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    if let Ok(kb) = parts[3].parse::<u64>() {
                        return Some(kb * 1024);
                    }
                }
            }
        }
        None
    }

    /// Query detailed memory info from /proc/meminfo over ADB
    pub async fn query_meminfo(&self) -> Result<DeviceMemoryInfo, PocketBridgeError> {
        let output = Command::new("adb")
            .args(["shell", "cat /proc/meminfo"])
            .output()
            .await
            .map_err(|e| PocketBridgeError::AdbError(format!("Failed to query /proc/meminfo: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PocketBridgeError::AdbError(format!("cat /proc/meminfo failed: {stderr}")));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut total = 0u64;
        let mut avail = 0u64;
        let mut swap_total = 0u64;
        let mut swap_free = 0u64;

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if parts[0] == "MemTotal:" {
                    total = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                } else if parts[0] == "MemAvailable:" {
                    avail = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                } else if parts[0] == "SwapTotal:" {
                    swap_total = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                } else if parts[0] == "SwapFree:" {
                    swap_free = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                }
            }
        }

        Ok(DeviceMemoryInfo {
            total_bytes: total,
            available_bytes: avail,
            swap_total_bytes: swap_total,
            swap_free_bytes: swap_free,
        })
    }

    /// Query battery level, battery temperature (in Celsius), and charging state from dumpsys battery over ADB
    pub async fn query_battery(&self) -> Result<(u32, f32, bool), PocketBridgeError> {
        let output = Command::new("adb")
            .args(["shell", "dumpsys battery"])
            .output()
            .await
            .map_err(|e| PocketBridgeError::AdbError(format!("Failed to query dumpsys battery: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PocketBridgeError::AdbError(format!("dumpsys battery failed: {stderr}")));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut level = 100u32;
        let mut temp_c = 25.0f32;
        let mut charging = false;

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("level:") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    level = parts[1].parse().unwrap_or(100);
                }
            } else if trimmed.starts_with("temperature:") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    let raw_temp: f32 = parts[1].parse().unwrap_or(250.0);
                    temp_c = raw_temp / 10.0;
                }
            } else if trimmed.starts_with("status:") {
                // Status 2 is BATTERY_STATUS_CHARGING, status 5 is BATTERY_STATUS_FULL
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    let s: u32 = parts[1].parse().unwrap_or(0);
                    charging = s == 2 || s == 5;
                }
            }
        }

        Ok((level, temp_c, charging))
    }
}

/// Physical memory characteristics read from /proc/meminfo on device
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceMemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_free_bytes: u64,
}
