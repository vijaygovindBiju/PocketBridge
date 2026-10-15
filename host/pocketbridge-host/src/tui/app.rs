#![allow(dead_code)]
use std::time::Instant;
use pocketbridge_core::{
    DeviceProfile, PayloadSizeResult, PluginManifest, PluginManager, SchedulingDecision,
};
use super::telemetry::{HostMetrics, TransportTelemetry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveScreen {
    Overview = 0,
    Devices = 1,
    Tasks = 2,
    Benchmarks = 3,
    Scheduler = 4,
    Plugins = 5,
    Logs = 6,
}

impl ActiveScreen {
    pub fn all() -> &'static [ActiveScreen] {
        &[
            ActiveScreen::Overview,
            ActiveScreen::Devices,
            ActiveScreen::Tasks,
            ActiveScreen::Benchmarks,
            ActiveScreen::Scheduler,
            ActiveScreen::Plugins,
            ActiveScreen::Logs,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            ActiveScreen::Overview => "Overview",
            ActiveScreen::Devices => "Devices",
            ActiveScreen::Tasks => "Tasks",
            ActiveScreen::Benchmarks => "Benchmarks",
            ActiveScreen::Scheduler => "Scheduler",
            ActiveScreen::Plugins => "Plugins",
            ActiveScreen::Logs => "Logs",
        }
    }

    pub fn shortcut(&self) -> char {
        match self {
            ActiveScreen::Overview => '1',
            ActiveScreen::Devices => '2',
            ActiveScreen::Tasks => '3',
            ActiveScreen::Benchmarks => '4',
            ActiveScreen::Scheduler => '5',
            ActiveScreen::Plugins => '6',
            ActiveScreen::Logs => '7',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkloadClassification {
    Synthetic,
    Benchmark,
    Simulated,
    RealInference,
    RealComputation,
    Unsupported,
}

impl WorkloadClassification {
    pub fn label(&self) -> &'static str {
        match self {
            WorkloadClassification::Synthetic => "Synthetic Kernel",
            WorkloadClassification::Benchmark => "Benchmark",
            WorkloadClassification::Simulated => "Simulated",
            WorkloadClassification::RealInference => "Real Inference",
            WorkloadClassification::RealComputation => "Real Computation",
            WorkloadClassification::Unsupported => "Unsupported",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Local,
    Remote,
    Hybrid,
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::Local => write!(f, "Host-Only"),
            ExecutionMode::Remote => write!(f, "Phone-Only"),
            ExecutionMode::Hybrid => write!(f, "Hybrid (Co-Proc)"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct TaskTimingBreakdown {
    pub upload_ms: Option<f64>,
    pub compute_ms: Option<f64>,
    pub download_ms: Option<f64>,
    pub total_ms: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub id: String,
    pub adapter_id: String,
    pub name: String,
    pub classification: WorkloadClassification,
    pub mode: ExecutionMode,
    pub target_device: String,
    pub status: TaskStatus,
    pub duration_ms: Option<f64>,
    pub records: u64,
    pub speedup: Option<f64>,
    pub sha256: Option<String>,
    pub error: Option<String>,
    pub started_at: chrono::DateTime<chrono::Local>,
    pub timings: Option<TaskTimingBreakdown>,
    pub parameters_summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSeverity {
    Info,
    Warn,
    Error,
    Debug,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub severity: LogSeverity,
    pub component: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum ActiveModal {
    None,
    Help,
    ActionPalette,
    TaskDetail(usize),
    ConfirmAction {
        title: String,
        message: String,
        action: String,
    },
}

#[derive(Debug, Clone)]
pub struct ActionItem {
    pub title: &'static str,
    pub description: &'static str,
    pub shortcut: &'static str,
    pub command_id: &'static str,
}

pub struct App {
    pub screen: ActiveScreen,
    pub modal: ActiveModal,
    pub port: u16,

    // Host & Transport Metrics
    pub host: HostMetrics,
    pub transport: TransportTelemetry,
    pub host_profile: Option<DeviceProfile>,
    pub worker_profile: Option<DeviceProfile>,

    // Selected table rows
    pub selected_device_index: usize,
    pub selected_task_index: usize,
    pub selected_benchmark_index: usize,
    pub selected_plugin_index: usize,
    pub selected_log_index: usize,
    pub selected_action_index: usize,

    // Tasks & Jobs
    pub tasks: Vec<TaskRecord>,
    pub active_task_id: Option<String>,
    pub active_task_progress: f32,
    pub active_task_status_msg: String,

    // Benchmarks
    pub benchmark_running: bool,
    pub benchmark_progress: (usize, usize),
    pub benchmark_stage: String,
    pub benchmark_runs: u32,
    pub benchmark_results: Vec<PayloadSizeResult>,
    pub last_saved_report: Option<String>,

    // Scheduler
    pub scheduler_records: u32,
    pub scheduler_rounds: u32,
    pub scheduler_host_load: f64,
    pub scheduler_margin: f64,
    pub scheduler_decision: Option<SchedulingDecision>,

    // Plugins
    pub installed_plugins: Vec<PluginManifest>,
    pub catalog_plugins: Vec<(PluginManifest, bool)>,

    // Logs & Search
    pub logs: Vec<LogEntry>,
    pub log_search_query: String,
    pub is_searching_logs: bool,
    pub log_severity_filter: Option<LogSeverity>,

    // Action Palette
    pub action_palette_query: String,

    // Graceful status note
    pub status_notification: Option<(String, Instant)>,
}

impl App {
    pub fn new() -> Self {
        let port = pocketbridge_core::DEFAULT_PORT;
        let mut app = Self {
            screen: ActiveScreen::Overview,
            modal: ActiveModal::None,
            port,

            host: HostMetrics::default(),
            transport: TransportTelemetry::new(port),
            host_profile: None,
            worker_profile: None,

            selected_device_index: 0,
            selected_task_index: 0,
            selected_benchmark_index: 0,
            selected_plugin_index: 0,
            selected_log_index: 0,
            selected_action_index: 0,

            tasks: Vec::new(),
            active_task_id: None,
            active_task_progress: 0.0,
            active_task_status_msg: String::new(),

            benchmark_running: false,
            benchmark_progress: (0, 0),
            benchmark_stage: String::new(),
            benchmark_runs: 20,
            benchmark_results: Vec::new(),
            last_saved_report: None,

            scheduler_records: 50_000,
            scheduler_rounds: 20,
            scheduler_host_load: 1.0,
            scheduler_margin: 1.15,
            scheduler_decision: None,

            installed_plugins: Vec::new(),
            catalog_plugins: Vec::new(),

            logs: Vec::new(),
            log_search_query: String::new(),
            is_searching_logs: false,
            log_severity_filter: None,

            action_palette_query: String::new(),
            status_notification: None,
        };

        app.refresh_plugins();
        app.log(LogSeverity::Info, "system", "PocketBridge production TUI initialized.");
        app
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_notification = Some((msg.into(), Instant::now()));
    }

    pub fn current_status(&self) -> Option<&str> {
        if let Some((msg, created)) = &self.status_notification {
            if created.elapsed().as_secs() < 4 {
                return Some(msg.as_str());
            }
        }
        None
    }

    pub fn log(&mut self, severity: LogSeverity, component: &str, msg: impl Into<String>) {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        self.logs.push(LogEntry {
            timestamp,
            severity,
            component: component.to_string(),
            message: msg.into(),
        });
        if self.logs.len() > 300 {
            self.logs.remove(0);
        }
    }

    pub fn refresh_plugins(&mut self) {
        let manager = PluginManager::default_manager();
        self.installed_plugins = manager.list_installed();
        self.catalog_plugins = manager.list_all_catalog_and_planned();
    }

    pub fn next_screen(&mut self) {
        let all = ActiveScreen::all();
        let idx = self.screen as usize;
        self.screen = all[(idx + 1) % all.len()];
    }

    pub fn prev_screen(&mut self) {
        let all = ActiveScreen::all();
        let idx = self.screen as usize;
        self.screen = all[(idx + all.len() - 1) % all.len()];
    }

    pub fn actions_list() -> Vec<ActionItem> {
        vec![
            ActionItem {
                title: "Overview Screen",
                description: "Inspect overall host, worker, transport & active workloads",
                shortcut: "1",
                command_id: "screen_overview",
            },
            ActionItem {
                title: "Devices Screen",
                description: "Inspect connected phones, hardware profiles & telemetry",
                shortcut: "2",
                command_id: "screen_devices",
            },
            ActionItem {
                title: "Tasks Screen",
                description: "Inspect task execution history, timings & SHA-256 verifications",
                shortcut: "3",
                command_id: "screen_tasks",
            },
            ActionItem {
                title: "Benchmarks Screen",
                description: "Run reproducible USB transport, SIMD, and memory bandwidth tests",
                shortcut: "4",
                command_id: "screen_benchmarks",
            },
            ActionItem {
                title: "Scheduler Screen",
                description: "Inspect explainable cost model, constraints & speedup predictions",
                shortcut: "5",
                command_id: "screen_scheduler",
            },
            ActionItem {
                title: "Plugins Hub",
                description: "Manage modular community adapters and pre-flight gates",
                shortcut: "6",
                command_id: "screen_plugins",
            },
            ActionItem {
                title: "Diagnostics & Logs",
                description: "Searchable event stream and systems diagnostics",
                shortcut: "7",
                command_id: "screen_logs",
            },
            ActionItem {
                title: "Connect / Handshake Phone",
                description: "Setup ADB port forwarding and verify worker handshake",
                shortcut: "c",
                command_id: "action_connect",
            },
            ActionItem {
                title: "Refresh Telemetry",
                description: "Poll host /proc and query phone heartbeat ping",
                shortcut: "r",
                command_id: "action_refresh",
            },
            ActionItem {
                title: "Run Fast Calibration Benchmark",
                description: "Execute 50,000 records SIMD chunk_transform across USB",
                shortcut: "j",
                command_id: "action_run_job",
            },
            ActionItem {
                title: "Disconnect Worker",
                description: "Safely unregister worker connection session",
                shortcut: "d",
                command_id: "action_disconnect",
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_navigation_wrap() {
        let mut app = App::new();
        assert_eq!(app.screen, ActiveScreen::Overview);

        app.next_screen();
        assert_eq!(app.screen, ActiveScreen::Devices);

        app.next_screen();
        assert_eq!(app.screen, ActiveScreen::Tasks);

        app.prev_screen();
        assert_eq!(app.screen, ActiveScreen::Devices);

        app.prev_screen();
        assert_eq!(app.screen, ActiveScreen::Overview);

        // Previous from Overview wraps to Logs
        app.prev_screen();
        assert_eq!(app.screen, ActiveScreen::Logs);
    }

    #[test]
    fn test_workload_classification_honesty() {
        let synth = WorkloadClassification::Synthetic;
        let infer = WorkloadClassification::RealInference;

        assert_ne!(synth.label(), infer.label());
        assert_eq!(synth.label(), "Synthetic Kernel");
        assert_eq!(infer.label(), "Real Inference");
    }

    #[test]
    fn test_action_palette_actions() {
        let actions = App::actions_list();
        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| a.command_id == "action_connect"));
        assert!(actions.iter().any(|a| a.command_id == "screen_benchmarks"));
    }

    #[test]
    fn test_stale_telemetry_age() {
        let mut t = TransportTelemetry::new(9876);
        assert!(t.is_stale());
        assert_eq!(t.age_string(), "No samples");

        t.last_heartbeat_time = Some(Instant::now());
        assert!(!t.is_stale());
        assert_eq!(t.age_string(), "<1s ago");
    }

    #[test]
    fn test_task_recording_state() {
        let mut app = App::new();
        assert!(app.tasks.is_empty());

        app.tasks.push(TaskRecord {
            id: "test-01".into(),
            adapter_id: "chunk_transform.v1".into(),
            name: "Calibration Test".into(),
            classification: WorkloadClassification::Benchmark,
            mode: ExecutionMode::Hybrid,
            target_device: "Galaxy S25 Ultra".into(),
            status: TaskStatus::Succeeded,
            duration_ms: Some(125.4),
            records: 50_000,
            speedup: Some(1.72),
            sha256: Some("abcdef123456".into()),
            error: None,
            started_at: chrono::Local::now(),
            timings: None,
            parameters_summary: "50,000 records".into(),
        });

        assert_eq!(app.tasks.len(), 1);
        assert_eq!(app.tasks[0].status, TaskStatus::Succeeded);
        assert_eq!(app.tasks[0].classification, WorkloadClassification::Benchmark);
    }
}
