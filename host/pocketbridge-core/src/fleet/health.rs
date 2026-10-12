use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::protocol::{read_envelope, write_envelope, Envelope, HeartbeatPayload, MessageKind};
use super::manager::{FleetManager, WorkerNodeState};
use super::work_stealing::FleetWorkQueue;

pub struct FleetHealthMonitor;

impl FleetHealthMonitor {
    /// Ping a worker port and return true if responsive within the specified timeout.
    pub async fn check_node_liveness(port: u16, timeout_duration: Duration) -> bool {
        let connect_fut = TcpStream::connect(("127.0.0.1", port));
        let mut stream = match timeout(timeout_duration, connect_fut).await {
            Ok(Ok(s)) => s,
            _ => return false,
        };

        let hb = Envelope::heartbeat(HeartbeatPayload {
            uptime_ms: 0,
            cpu_usage_percent: None,
            memory_used_bytes: None,
            memory_total_bytes: None,
            temperature_celsius: None,
            battery_percent: None,
            battery_charging: None,
            thermal_status: None,
        });

        if write_envelope(&mut stream, &hb).await.is_err() {
            return false;
        }

        match timeout(timeout_duration, read_envelope(&mut stream)).await {
            Ok(Ok(env)) => env.kind == MessageKind::Heartbeat,
            _ => false,
        }
    }

    /// Audit all fleet nodes, update their states, and recover orphaned tasks.
    pub async fn audit_fleet_health(
        fleet: &mut FleetManager,
        queue: &mut FleetWorkQueue,
        timeout_duration: Duration,
    ) -> usize {
        let mut recovered_chunks = 0;

        for node in &mut fleet.nodes {
            if node.state == WorkerNodeState::Failed || node.state == WorkerNodeState::Disconnected {
                continue;
            }

            let is_alive = Self::check_node_liveness(node.port, timeout_duration).await;
            if !is_alive {
                warn!("Worker node {} (port {}) is unresponsive. Initiating failover.", node.serial, node.port);
                node.state = WorkerNodeState::Disconnected;
                node.failed_chunks += 1;
                let requeued = queue.fail_worker_chunks(&node.serial);
                recovered_chunks += requeued;
                debug!("Recovered {} orphaned chunks from failed worker {}", requeued, node.serial);
            }
        }

        recovered_chunks
    }
}
