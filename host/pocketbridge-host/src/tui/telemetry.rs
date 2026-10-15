#![allow(dead_code)]
use std::time::Instant;
use tokio::fs;
use pocketbridge_core::{AdbTransport, HeartbeatPayload, HelloPayload, MessageKind, read_envelope, write_envelope, Envelope};

/// Real Host metrics collected directly from Linux /proc filesystem
#[derive(Debug, Clone, Default)]
pub struct HostMetrics {
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub cpu_model: String,
    pub cpu_cores: u32,
    pub cpu_usage_pct: f64,
    pub ram_total_bytes: u64,
    pub ram_used_bytes: u64,
    pub ram_available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub load_1m: f64,
    pub load_5m: f64,
    pub load_15m: f64,
    pub last_updated: Option<chrono::DateTime<chrono::Local>>,
}

/// Last raw CPU ticks from /proc/stat to compute differential usage
#[derive(Debug, Clone, Default)]
pub struct CpuSample {
    pub total: u64,
    pub idle: u64,
}

impl HostMetrics {
    /// Poll current Linux host metrics from /proc
    pub async fn poll(last_cpu: &mut Option<CpuSample>) -> Self {
        let meminfo = fs::read_to_string("/proc/meminfo").await.unwrap_or_default();
        let loadavg = fs::read_to_string("/proc/loadavg").await.unwrap_or_default();
        let stat = fs::read_to_string("/proc/stat").await.unwrap_or_default();

        let ram_total = parse_meminfo_kb(&meminfo, "MemTotal").unwrap_or(0) * 1024;
        let ram_avail = parse_meminfo_kb(&meminfo, "MemAvailable").unwrap_or(0) * 1024;
        let ram_used = ram_total.saturating_sub(ram_avail);

        let swap_total = parse_meminfo_kb(&meminfo, "SwapTotal").unwrap_or(0) * 1024;
        let swap_free = parse_meminfo_kb(&meminfo, "SwapFree").unwrap_or(0) * 1024;
        let swap_used = swap_total.saturating_sub(swap_free);

        let (l1, l5, l15) = parse_loadavg(&loadavg);
        let cpu_usage = calculate_cpu_usage(&stat, last_cpu);

        let cpu_model = fs::read_to_string("/proc/cpuinfo")
            .await
            .ok()
            .and_then(|info| {
                info.lines()
                    .find(|l| l.starts_with("model name"))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or_else(|| "x86_64 CPU".to_string());

        let cpu_cores = fs::read_to_string("/proc/cpuinfo")
            .await
            .ok()
            .map(|info| info.lines().filter(|l| l.starts_with("processor")).count() as u32)
            .unwrap_or(1);

        let hostname = fs::read_to_string("/etc/hostname")
            .await
            .unwrap_or_else(|_| "archlinux".into())
            .trim()
            .to_string();

        let kernel = fs::read_to_string("/proc/sys/kernel/osrelease")
            .await
            .unwrap_or_else(|_| "Linux".into())
            .trim()
            .to_string();

        Self {
            hostname,
            os: "Arch Linux".to_string(),
            kernel,
            cpu_model,
            cpu_cores,
            cpu_usage_pct: cpu_usage,
            ram_total_bytes: ram_total,
            ram_used_bytes: ram_used,
            ram_available_bytes: ram_avail,
            swap_total_bytes: swap_total,
            swap_used_bytes: swap_used,
            load_1m: l1,
            load_5m: l5,
            load_15m: l15,
            last_updated: Some(chrono::Local::now()),
        }
    }
}

fn parse_meminfo_kb(meminfo: &str, key: &str) -> Option<u64> {
    meminfo
        .lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
}

fn parse_loadavg(loadavg: &str) -> (f64, f64, f64) {
    let mut parts = loadavg.split_whitespace();
    let l1 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let l5 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let l15 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    (l1, l5, l15)
}

fn calculate_cpu_usage(stat: &str, last_sample: &mut Option<CpuSample>) -> f64 {
    if let Some(line) = stat.lines().find(|l| l.starts_with("cpu ")) {
        let parts: Vec<u64> = line
            .split_whitespace()
            .skip(1)
            .filter_map(|s| s.parse().ok())
            .collect();
        if parts.len() >= 4 {
            let user = parts[0];
            let nice = parts[1];
            let system = parts[2];
            let idle = parts[3];
            let iowait = parts.get(4).copied().unwrap_or(0);
            let irq = parts.get(5).copied().unwrap_or(0);
            let softirq = parts.get(6).copied().unwrap_or(0);
            let steal = parts.get(7).copied().unwrap_or(0);

            let current_idle = idle + iowait;
            let current_total = user + nice + system + current_idle + irq + softirq + steal;

            if let Some(last) = last_sample {
                let total_diff = current_total.saturating_sub(last.total);
                let idle_diff = current_idle.saturating_sub(last.idle);

                last.total = current_total;
                last.idle = current_idle;

                if total_diff > 0 {
                    let active_diff = total_diff.saturating_sub(idle_diff);
                    return ((active_diff as f64 / total_diff as f64) * 100.0).clamp(0.0, 100.0);
                }
            } else {
                *last_sample = Some(CpuSample {
                    total: current_total,
                    idle: current_idle,
                });
            }
        }
    }
    0.0
}

/// Transport & Worker link telemetry collected over USB / ADB
#[derive(Debug, Clone)]
pub struct TransportTelemetry {
    pub is_connected: bool,
    pub is_connecting: bool,
    pub adb_device: Option<String>,
    pub port: u16,
    pub rtt_ms: Option<f64>,
    pub measured_throughput_mbps: Option<f64>,
    pub throughput_sample_bytes: Option<usize>,
    pub last_heartbeat: Option<HeartbeatPayload>,
    pub last_heartbeat_time: Option<Instant>,
    pub hello: Option<HelloPayload>,
    pub last_error: Option<String>,
}

impl TransportTelemetry {
    pub fn new(port: u16) -> Self {
        Self {
            is_connected: false,
            is_connecting: false,
            adb_device: None,
            port,
            rtt_ms: None,
            measured_throughput_mbps: None,
            throughput_sample_bytes: None,
            last_heartbeat: None,
            last_heartbeat_time: None,
            hello: None,
            last_error: None,
        }
    }

    /// Checks if telemetry data has become stale (> 5 seconds since last heartbeat)
    pub fn is_stale(&self) -> bool {
        match self.last_heartbeat_time {
            Some(t) => t.elapsed().as_secs() > 5,
            None => true,
        }
    }

    /// Formatted age string for last heartbeat
    pub fn age_string(&self) -> String {
        match self.last_heartbeat_time {
            Some(t) => {
                let s = t.elapsed().as_secs();
                if s == 0 {
                    "<1s ago".to_string()
                } else if s > 5 {
                    format!("Stale ({}s ago)", s)
                } else {
                    format!("{}s ago", s)
                }
            }
            None => "No samples".to_string(),
        }
    }

    /// Query one heartbeat ping over ADB port to measure RTT and fetch worker telemetry
    pub async fn query_heartbeat(&mut self) -> Result<HeartbeatPayload, String> {
        let transport = AdbTransport::new(self.port);
        let t_start = Instant::now();

        let mut stream = transport.connect_tcp().await.map_err(|e| e.to_string())?;

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

        write_envelope(&mut stream, &hb).await.map_err(|e| e.to_string())?;
        let reply = read_envelope(&mut stream).await.map_err(|e| e.to_string())?;

        let rtt = t_start.elapsed().as_secs_f64() * 1000.0;
        self.rtt_ms = Some(rtt);

        if reply.kind == MessageKind::Heartbeat {
            let worker_hb = serde_json::from_value::<HeartbeatPayload>(reply.payload).map_err(|e| e.to_string())?;
            self.last_heartbeat = Some(worker_hb.clone());
            self.last_heartbeat_time = Some(Instant::now());
            self.is_connected = true;
            self.last_error = None;
            Ok(worker_hb)
        } else {
            Err(format!("Unexpected reply kind: {:?}", reply.kind))
        }
    }
}
