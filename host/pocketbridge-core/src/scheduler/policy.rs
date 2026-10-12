use super::types::{ConstraintEvaluation, CostEstimate, ExecutionTarget, SchedulerConfig, SchedulingDecision};

/// Policy decision engine enforcing constraints and speedup margins.
pub struct PolicyEngine;

impl PolicyEngine {
    /// Evaluate cost estimates and constraints to produce an explainable scheduling decision.
    pub fn evaluate(
        estimates: CostEstimate,
        constraints: ConstraintEvaluation,
        config: &SchedulerConfig,
    ) -> SchedulingDecision {
        // 1. Check Hardware Safety Constraints First
        if !constraints.link_ok {
            let reason = match constraints.link_rtt_ms {
                Some(rtt) => format!(
                    "Keep on local host: ADB link latency ({:.1}ms) exceeds maximum acceptable threshold ({:.1}ms)",
                    rtt, config.max_acceptable_rtt_ms
                ),
                None => "Keep on local host: No active ADB link detected with Android worker".to_string(),
            };
            return SchedulingDecision {
                target: ExecutionTarget::LocalHost,
                approved_for_offload: false,
                estimates,
                constraints,
                decision_reason: reason,
                speedup_margin: config.speedup_margin,
            };
        }

        if !constraints.battery_ok {
            let pct = constraints.battery_percent.unwrap_or(0);
            let reason = format!(
                "Keep on local host: Phone battery is low ({}%, discharging) - below safety threshold ({}%)",
                pct, config.min_battery_percent
            );
            return SchedulingDecision {
                target: ExecutionTarget::LocalHost,
                approved_for_offload: false,
                estimates,
                constraints,
                decision_reason: reason,
                speedup_margin: config.speedup_margin,
            };
        }

        if !constraints.thermal_ok {
            let status = constraints.thermal_status.as_deref().unwrap_or("severe");
            let reason = format!(
                "Keep on local host: Phone thermal status is '{}' - thermal throttling protection active",
                status
            );
            return SchedulingDecision {
                target: ExecutionTarget::LocalHost,
                approved_for_offload: false,
                estimates,
                constraints,
                decision_reason: reason,
                speedup_margin: config.speedup_margin,
            };
        }

        if !constraints.memory_ok {
            let req_mb = constraints.memory_required_bytes as f64 / (1024.0 * 1024.0);
            let avail_mb = constraints.memory_available_bytes.unwrap_or(0) as f64 / (1024.0 * 1024.0);
            let reason = format!(
                "Keep on local host: Task memory requirement ({:.1} MB) exceeds safe phone budget ({:.1} MB available)",
                req_mb, avail_mb
            );
            return SchedulingDecision {
                target: ExecutionTarget::LocalHost,
                approved_for_offload: false,
                estimates,
                constraints,
                decision_reason: reason,
                speedup_margin: config.speedup_margin,
            };
        }

        // 2. Evaluate Analytical Speedup Against Configured Margin
        if estimates.predicted_speedup >= config.speedup_margin {
            let reason = format!(
                "Offload to phone worker: Predicted {:.2}x speedup (remote {:.1}ms vs local {:.1}ms) exceeds {:.2}x policy margin",
                estimates.predicted_speedup, estimates.remote_total_ms, estimates.local_compute_ms, config.speedup_margin
            );
            SchedulingDecision {
                target: ExecutionTarget::RemoteWorker,
                approved_for_offload: true,
                estimates,
                constraints,
                decision_reason: reason,
                speedup_margin: config.speedup_margin,
            }
        } else {
            let reason = format!(
                "Keep on local host: Predicted remote speedup ({:.2}x) does not justify link overhead (policy requires >= {:.2}x; local {:.1}ms vs remote {:.1}ms)",
                estimates.predicted_speedup, config.speedup_margin, estimates.local_compute_ms, estimates.remote_total_ms
            );
            SchedulingDecision {
                target: ExecutionTarget::LocalHost,
                approved_for_offload: false,
                estimates,
                constraints,
                decision_reason: reason,
                speedup_margin: config.speedup_margin,
            }
        }
    }
}
