use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Created,
    Queued,
    RunningLocal,
    Sending,
    RunningRemote,
    Receiving,
    ValidatingResult,
    Succeeded,
    Failed,
    Cancelled,
}

impl std::fmt::Display for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobState::Created => write!(f, "Created"),
            JobState::Queued => write!(f, "Queued"),
            JobState::RunningLocal => write!(f, "Running Locally"),
            JobState::Sending => write!(f, "Uploading to Phone"),
            JobState::RunningRemote => write!(f, "Executing on Phone"),
            JobState::Receiving => write!(f, "Downloading Results"),
            JobState::ValidatingResult => write!(f, "Validating Output"),
            JobState::Succeeded => write!(f, "Succeeded"),
            JobState::Failed => write!(f, "Failed"),
            JobState::Cancelled => write!(f, "Cancelled"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobExecutionResult {
    pub job_id: String,
    pub adapter_id: String,
    pub state: JobState,
    pub local_duration_ms: Option<f64>,
    pub remote_upload_ms: Option<f64>,
    pub remote_compute_ms: Option<f64>,
    pub remote_download_ms: Option<f64>,
    pub remote_total_ms: Option<f64>,
    pub speedup: Option<f64>,
    pub local_sha256: Option<String>,
    pub remote_sha256: Option<String>,
    pub outputs_match: bool,
    pub records_processed: u64,
    pub error_message: Option<String>,
}

impl JobExecutionResult {
    pub fn calculate_speedup(&mut self) {
        if let (Some(local), Some(remote_total)) = (self.local_duration_ms, self.remote_total_ms) {
            if remote_total > 0.0 {
                self.speedup = Some(local / remote_total);
            }
        }
    }
}
