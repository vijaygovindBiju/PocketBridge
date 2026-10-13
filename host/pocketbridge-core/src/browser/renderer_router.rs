use serde::{Deserialize, Serialize};

/// Classification of a browser tab's execution priority and location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabExecutionTarget {
    /// Active tab with focused viewport: executed locally on laptop with 60+ FPS zero-latency UI
    HostActive,
    /// Inactive / background tab: offloaded to phone's Android WebView / headless engine
    WorkerBackground,
}

/// Information describing an open browser tab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserTabDescriptor {
    pub tab_id: u32,
    pub title: String,
    pub url: String,
    pub is_active: bool,
    pub is_audible: bool,
    pub memory_estimate_bytes: u64,
}

/// Evaluates open browser tabs and routes them according to focus and resource constraints.
pub struct BrowserRendererRouter;

impl BrowserRendererRouter {
    /// Classify a tab into HostActive or WorkerBackground.
    pub fn classify_tab(tab: &BrowserTabDescriptor) -> TabExecutionTarget {
        // Active foreground tab MUST stay on host laptop for zero UI/input lag
        if tab.is_active {
            TabExecutionTarget::HostActive
        } else {
            TabExecutionTarget::WorkerBackground
        }
    }

    /// Split a list of tabs into host tabs and phone offloaded tabs.
    pub fn route_tabs(
        tabs: &[BrowserTabDescriptor],
    ) -> (Vec<BrowserTabDescriptor>, Vec<BrowserTabDescriptor>) {
        let mut host_tabs = Vec::new();
        let mut worker_tabs = Vec::new();

        for tab in tabs {
            match Self::classify_tab(tab) {
                TabExecutionTarget::HostActive => host_tabs.push(tab.clone()),
                TabExecutionTarget::WorkerBackground => worker_tabs.push(tab.clone()),
            }
        }

        (host_tabs, worker_tabs)
    }

    /// Calculate estimated RAM savings on host laptop (in bytes and percentage).
    pub fn calculate_ram_savings(
        host_tabs: &[BrowserTabDescriptor],
        worker_tabs: &[BrowserTabDescriptor],
    ) -> (u64, f64) {
        let host_ram: u64 = host_tabs.iter().map(|t| t.memory_estimate_bytes).sum();
        let saved_ram: u64 = worker_tabs.iter().map(|t| t.memory_estimate_bytes).sum();
        let total_ram = host_ram + saved_ram;

        let pct = if total_ram > 0 {
            (saved_ram as f64 / total_ram as f64) * 100.0
        } else {
            0.0
        };

        (saved_ram, pct)
    }
}
