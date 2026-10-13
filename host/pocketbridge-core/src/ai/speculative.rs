use serde::{Deserialize, Serialize};

/// Result from speculative decoding round: small model on phone drafts, big model on laptop verifies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativeRoundResult {
    pub draft_tokens_count: usize,
    pub accepted_tokens_count: usize,
    pub draft_time_ms: f64,
    pub verify_time_ms: f64,
    pub effective_speedup: f64,
}

pub struct SpeculativeCoordinator;

impl SpeculativeCoordinator {
    /// Calculate effective speedup from draft generation and parallel verification.
    pub fn evaluate_speedup(
        draft_tokens: usize,
        accepted_tokens: usize,
        draft_time_ms: f64,
        verify_time_ms: f64,
        target_token_time_ms: f64,
    ) -> SpeculativeRoundResult {
        let total_time_ms = draft_time_ms + verify_time_ms;
        let baseline_time_ms = (accepted_tokens as f64) * target_token_time_ms;
        let speedup = if total_time_ms > 0.0 {
            baseline_time_ms / total_time_ms
        } else {
            1.0
        };

        SpeculativeRoundResult {
            draft_tokens_count: draft_tokens,
            accepted_tokens_count: accepted_tokens,
            draft_time_ms,
            verify_time_ms,
            effective_speedup: speedup.max(1.0),
        }
    }
}
