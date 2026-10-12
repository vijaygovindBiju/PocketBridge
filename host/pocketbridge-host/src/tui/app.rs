use pocketbridge_core::{
    DeviceProfile, HeartbeatPayload, HelloPayload, JobExecutionResult, PayloadSizeResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Dashboard = 0,
    Benchmark = 1,
    Profile = 2,
}

pub struct App {
    pub active_tab: ActiveTab,
    pub port: u16,
    pub runs: u32,

    // Connection state
    pub connected: bool,
    pub connecting: bool,
    pub status_message: String,
    pub device_name: Option<String>,
    pub worker_hello: Option<HelloPayload>,
    pub last_heartbeat: Option<HeartbeatPayload>,

    // Hardware profiles
    pub host_profile: Option<DeviceProfile>,
    pub worker_profile: Option<DeviceProfile>,

    // Benchmark state
    pub benchmark_running: bool,
    pub benchmark_stage: String,
    pub benchmark_progress: (usize, usize),
    pub benchmark_results: Vec<PayloadSizeResult>,
    pub last_saved_report: Option<String>,

    // Job execution state (Phase 1 & 2)
    pub job_running: bool,
    pub job_records: u32,
    pub job_rounds: u32,
    pub job_progress_pct: f32,
    pub job_status_msg: String,
    pub active_job_result: Option<JobExecutionResult>,

    // Activity logs
    pub logs: Vec<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            active_tab: ActiveTab::Dashboard,
            port: pocketbridge_core::DEFAULT_PORT,
            runs: 20,

            connected: false,
            connecting: false,
            status_message: "Ready to connect".into(),
            device_name: None,
            worker_hello: None,
            last_heartbeat: None,

            host_profile: None,
            worker_profile: None,

            benchmark_running: false,
            benchmark_stage: String::new(),
            benchmark_progress: (0, 0),
            benchmark_results: Vec::new(),
            last_saved_report: None,

            job_running: false,
            job_records: 50_000,
            job_rounds: 20,
            job_progress_pct: 0.0,
            job_status_msg: "Press 'j' to run compute job".into(),
            active_job_result: None,

            logs: vec!["PocketBridge initialized. Press 'c' to connect, 'j' to offload compute job.".into()],
        }
    }

    pub fn log(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        let timestamp = chrono::Local::now().format("%H:%M:%S");
        self.logs.push(format!("[{timestamp}] {msg}"));
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    pub fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Dashboard => ActiveTab::Benchmark,
            ActiveTab::Benchmark => ActiveTab::Profile,
            ActiveTab::Profile => ActiveTab::Dashboard,
        };
    }

    pub fn prev_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Dashboard => ActiveTab::Profile,
            ActiveTab::Benchmark => ActiveTab::Dashboard,
            ActiveTab::Profile => ActiveTab::Benchmark,
        };
    }
}
