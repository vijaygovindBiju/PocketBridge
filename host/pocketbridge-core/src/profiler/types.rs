use serde::{Deserialize, Serialize};

/// Unified device profile representing either host or worker system characteristics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub role: String, // "host" or "worker"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    pub os: String, // "Linux" or "Android"
    pub os_version: String, // Kernel version or Android version
    pub architecture: String, // "x86_64" or "aarch64"
    pub cpu_model: String,
    pub cpu_cores: u32,
    pub ram_total_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ram_available_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_total_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_available_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android_api_level: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery_percent: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery_charging: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thermal_status: Option<String>,
}

impl DeviceProfile {
    pub fn format_ram(bytes: u64) -> String {
        let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        format!("{:.2} GB", gb)
    }
}
