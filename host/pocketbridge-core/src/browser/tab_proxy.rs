use serde::{Deserialize, Serialize};

pub const BROWSER_ADAPTER_ID: &str = "browser_tab_offload.v1";

/// Payload sent over USB to render or maintain a background tab on the phone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabRenderPayload {
    pub tab_id: u32,
    pub url: String,
    pub title: String,
    pub execute_js: Option<String>,
}

/// Status report returned from the phone worker for an offloaded tab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabRenderStatus {
    pub tab_id: u32,
    pub status: String,
    pub dom_loaded: bool,
    pub memory_used_bytes: u64,
    pub eval_result: Option<String>,
}
