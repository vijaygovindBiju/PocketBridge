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
}
