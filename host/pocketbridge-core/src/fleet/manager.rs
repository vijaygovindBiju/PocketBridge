use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use crate::error::PocketBridgeError;
use crate::profiler::DeviceProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerNodeState {
    Connected,
    Busy,
    Disconnected,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerNode {
    pub serial: String,
    pub port: u16,
    pub state: WorkerNodeState,
    pub profile: Option<DeviceProfile>,
    pub completed_chunks: u32,
    pub failed_chunks: u32,
}

impl WorkerNode {
    pub fn new(serial: String, port: u16) -> Self {
        Self {
            serial,
            port,
            state: WorkerNodeState::Connected,
            profile: None,
            completed_chunks: 0,
            failed_chunks: 0,
        }
    }
}

pub struct FleetManager {
    pub base_port: u16,
    pub nodes: Vec<WorkerNode>,
}

impl Default for FleetManager {
    fn default() -> Self {
        Self::new(9876)
    }
}

impl FleetManager {
    pub fn new(base_port: u16) -> Self {
        Self {
            base_port,
            nodes: Vec::new(),
        }
    }

    pub fn add_node(&mut self, serial: String, port: u16) {
        if !self.nodes.iter().any(|n| n.serial == serial) {
            self.nodes.push(WorkerNode::new(serial, port));
        }
    }

    pub fn active_nodes(&self) -> Vec<&WorkerNode> {
        self.nodes
            .iter()
            .filter(|n| n.state == WorkerNodeState::Connected || n.state == WorkerNodeState::Busy)
            .collect()
    }

    pub fn mark_state(&mut self, serial: &str, state: WorkerNodeState) {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.serial == serial) {
            node.state = state;
        }
    }

    pub fn record_success(&mut self, serial: &str) {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.serial == serial) {
            node.completed_chunks += 1;
            node.state = WorkerNodeState::Connected;
        }
    }

    pub fn record_failure(&mut self, serial: &str) {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.serial == serial) {
            node.failed_chunks += 1;
            node.state = WorkerNodeState::Failed;
        }
    }

    /// Query adb devices -l and automatically configure per-device port forwards.
    pub async fn discover_and_setup(&mut self) -> Result<usize, PocketBridgeError> {
        let output = Command::new("adb")
            .arg("devices")
            .arg("-l")
            .output()
            .await
            .map_err(|e| PocketBridgeError::AdbError(format!("Failed to execute adb devices: {e}")))?;

        if !output.status.success() {
            return Err(PocketBridgeError::AdbError(
                "adb devices -l returned non-zero exit status".into(),
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut discovered = 0;

        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("List of devices") {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[1] == "device" {
                let serial = parts[0].to_string();
                let port = self.base_port + (discovered as u16);

                let forward_out = Command::new("adb")
                    .arg("-s")
                    .arg(&serial)
                    .arg("forward")
                    .arg(format!("tcp:{port}"))
                    .arg("tcp:9876")
                    .output()
                    .await;

                match forward_out {
                    Ok(res) if res.status.success() => {
                        info!("Configured ADB forward for {serial}: 127.0.0.1:{port} -> device:9876");
                        self.add_node(serial, port);
                        discovered += 1;
                    }
                    Ok(res) => {
                        warn!("Failed to forward port for {serial}: {}", String::from_utf8_lossy(&res.stderr));
                    }
                    Err(e) => {
                        error!("Error running adb forward for {serial}: {e}");
                    }
                }
            }
        }

        Ok(discovered)
    }

    pub async fn clean_forwarding(&self) {
        for node in &self.nodes {
            let _ = Command::new("adb")
                .arg("-s")
                .arg(&node.serial)
                .arg("forward")
                .arg("--remove")
                .arg(format!("tcp:{}", node.port))
                .output()
                .await;
            debug!("Removed forward for {} on port {}", node.serial, node.port);
        }
    }
}
