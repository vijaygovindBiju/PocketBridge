pub mod cost_model;
pub mod policy;
pub mod types;

pub use cost_model::{CostModel, PerformanceBaseline};
pub use policy::PolicyEngine;
pub use types::{
    ConstraintEvaluation, CostEstimate, ExecutionTarget, SchedulerConfig, SchedulingDecision,
};

use crate::profiler::DeviceProfile;

/// High-level Explainable Scheduler coordinating analytical cost modeling and safety policies.
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct Scheduler {
    pub config: SchedulerConfig,
    pub cost_model: CostModel,
}

impl Scheduler {
    pub fn new(config: SchedulerConfig, cost_model: CostModel) -> Self {
        Self { config, cost_model }
    }

    /// Evaluate an upcoming task against hardware profiles and analytical cost models.
    pub fn evaluate_task(
        &self,
        record_count: u32,
        rounds: u32,
        input_bytes: usize,
        output_bytes: usize,
        worker_profile: Option<&DeviceProfile>,
        link_rtt_ms: Option<f64>,
    ) -> SchedulingDecision {
        // 1. Calculate Analytical Cost Estimates
        let estimates = self.cost_model.estimate_chunk_transform(
            record_count,
            rounds,
            input_bytes,
            output_bytes,
            self.config.host_load_factor,
        );

        // 2. Evaluate Hardware Constraints
        let battery_percent = worker_profile.and_then(|p| p.battery_percent);
        let battery_charging = worker_profile.and_then(|p| p.battery_charging);
        let battery_ok = match (battery_percent, battery_charging) {
            (Some(pct), Some(false)) => pct >= self.config.min_battery_percent,
            (Some(_), Some(true)) => true, // Always OK if charging
            (Some(pct), None) => pct >= self.config.min_battery_percent,
            (None, _) => true, // Unknown / assume OK unless reported otherwise
        };

        let thermal_status = worker_profile.and_then(|p| p.thermal_status.clone());
        let thermal_ok = if self.config.reject_severe_thermal {
            !matches!(thermal_status.as_deref(), Some("severe") | Some("critical") | Some("emergency") | Some("shutdown"))
        } else {
            true
        };

        let memory_required_bytes = (input_bytes + output_bytes) as u64;
        let memory_available_bytes = worker_profile.and_then(|p| p.ram_available_bytes);
        let memory_ok = match memory_available_bytes {
            Some(avail) => memory_required_bytes < (avail / 2), // Require at least 2x headroom
            None => true,
        };

        let link_ok = match link_rtt_ms {
            Some(rtt) => rtt <= self.config.max_acceptable_rtt_ms,
            None => worker_profile.is_some(), // OK if worker profile was successfully fetched
        };

        let constraints = ConstraintEvaluation {
            battery_ok,
            battery_percent,
            battery_charging,
            thermal_ok,
            thermal_status,
            memory_ok,
            memory_required_bytes,
            memory_available_bytes,
            link_ok,
            link_rtt_ms,
        };

        // 3. Delegate to Policy Engine
        PolicyEngine::evaluate(estimates, constraints, &self.config)
    }

    /// Format a formatted text summary card of the scheduling decision.
    pub fn format_decision_card(decision: &SchedulingDecision) -> String {
        let mut out = String::new();
        out.push_str("==========================================================================\n");
        out.push_str("POCKETBRIDGE EXPLAINABLE SCHEDULER DECISION\n");
        out.push_str("==========================================================================\n");

        let badge = match decision.target {
            ExecutionTarget::RemoteWorker => "[RECOMMENDATION: OFFLOAD TO ANDROID WORKER]",
            ExecutionTarget::LocalHost => "[RECOMMENDATION: EXECUTE LOCALLY ON HOST]",
            ExecutionTarget::HybridSplit { .. } => "[RECOMMENDATION: HYBRID CO-PROCESSING SPLIT]",
        };
        out.push_str(&format!("Status:               {}\n", badge));
        out.push_str(&format!("Rationale:            {}\n", decision.decision_reason));
        out.push_str("--------------------------------------------------------------------------\n");
        out.push_str("Analytical Cost Estimates:\n");
        out.push_str(&format!("  Estimated Local Time:   {:.2} ms\n", decision.estimates.local_compute_ms));
        out.push_str(&format!("  Estimated Transfer:     {:.2} ms (In: {:.2}ms, Out: {:.2}ms)\n",
            decision.estimates.transfer_in_ms + decision.estimates.transfer_out_ms,
            decision.estimates.transfer_in_ms,
            decision.estimates.transfer_out_ms
        ));
        out.push_str(&format!("  Estimated Remote Comp:  {:.2} ms\n", decision.estimates.remote_compute_ms));
        out.push_str(&format!("  Estimated Remote Total: {:.2} ms\n", decision.estimates.remote_total_ms));
        out.push_str(&format!("  Predicted Speedup:      {:.2}x (Required Policy Margin: {:.2}x)\n",
            decision.estimates.predicted_speedup, decision.speedup_margin
        ));
        out.push_str("--------------------------------------------------------------------------\n");
        out.push_str("Hardware Safety Constraint Evaluation:\n");
        out.push_str(&format!("  Battery Constraint:     {} (Level: {:?}%, Charging: {:?})\n",
            if decision.constraints.battery_ok { "PASSED" } else { "FAILED" },
            decision.constraints.battery_percent.unwrap_or(0),
            decision.constraints.battery_charging.unwrap_or(false)
        ));
        out.push_str(&format!("  Thermal Constraint:     {} (Status: {})\n",
            if decision.constraints.thermal_ok { "PASSED" } else { "FAILED" },
            decision.constraints.thermal_status.as_deref().unwrap_or("normal")
        ));
        out.push_str(&format!("  Memory Headroom:        {} (Req: {:.1} MB)\n",
            if decision.constraints.memory_ok { "PASSED" } else { "FAILED" },
            decision.constraints.memory_required_bytes as f64 / (1024.0 * 1024.0)
        ));
        out.push_str(&format!("  Link Availability:      {} (RTT: {:.1} ms)\n",
            if decision.constraints.link_ok { "PASSED" } else { "FAILED" },
            decision.constraints.link_rtt_ms.unwrap_or(0.0)
        ));
        out.push_str("==========================================================================");
        out
    }
}
