use pocketbridge_core::{
    CostModel, DeviceProfile, ExecutionTarget, Scheduler, SchedulerConfig,
};

fn create_mock_worker_profile(
    battery_percent: Option<u8>,
    battery_charging: Option<bool>,
    thermal_status: Option<String>,
) -> DeviceProfile {
    DeviceProfile {
        role: "worker".into(),
        hostname: None,
        os: "Android".into(),
        os_version: "Android 17".into(),
        architecture: "arm64-v8a".into(),
        cpu_model: "Snapdragon 8 Elite".into(),
        cpu_cores: 8,
        ram_total_bytes: 12 * 1024 * 1024 * 1024,
        ram_available_bytes: Some(6 * 1024 * 1024 * 1024),
        gpu: None,
        storage_total_bytes: None,
        storage_available_bytes: None,
        android_api_level: Some(37),
        device_model: Some("Samsung Galaxy S25 Ultra".into()),
        soc: Some("SM8750".into()),
        battery_percent,
        battery_charging,
        thermal_status,
    }
}

#[test]
fn test_scheduler_speedup_evaluation() {
    let scheduler = Scheduler::default();
    let worker_profile = create_mock_worker_profile(Some(85), Some(true), Some("none".into()));

    // When laptop is idle, local compute is faster for small/medium jobs
    let decision = scheduler.evaluate_task(
        500_000,
        50,
        64 * 1024,
        64 * 1024,
        Some(&worker_profile),
        Some(2.0),
    );

    assert!(decision.constraints.all_passed());
    assert!(!decision.approved_for_offload);
    assert_eq!(decision.target, ExecutionTarget::LocalHost);
    assert!(decision.decision_reason.contains("Keep on local host"));
}

#[test]
fn test_scheduler_battery_rejection() {
    let scheduler = Scheduler::default();
    // 14% battery and discharging
    let worker_profile = create_mock_worker_profile(Some(14), Some(false), Some("none".into()));

    let decision = scheduler.evaluate_task(
        500_000,
        50,
        64 * 1024,
        64 * 1024,
        Some(&worker_profile),
        Some(2.0),
    );

    assert!(!decision.constraints.battery_ok);
    assert!(!decision.approved_for_offload);
    assert_eq!(decision.target, ExecutionTarget::LocalHost);
    assert!(decision.decision_reason.contains("Phone battery is low"));
}

#[test]
fn test_scheduler_battery_charging_ok() {
    let scheduler = Scheduler::default();
    // 14% battery but plugged into charger
    let worker_profile = create_mock_worker_profile(Some(14), Some(true), Some("none".into()));

    let decision = scheduler.evaluate_task(
        500_000,
        50,
        64 * 1024,
        64 * 1024,
        Some(&worker_profile),
        Some(2.0),
    );

    assert!(decision.constraints.battery_ok);
}

#[test]
fn test_scheduler_thermal_rejection() {
    let scheduler = Scheduler::default();
    let worker_profile = create_mock_worker_profile(Some(90), Some(true), Some("severe".into()));

    let decision = scheduler.evaluate_task(
        500_000,
        50,
        64 * 1024,
        64 * 1024,
        Some(&worker_profile),
        Some(2.0),
    );

    assert!(!decision.constraints.thermal_ok);
    assert!(!decision.approved_for_offload);
    assert_eq!(decision.target, ExecutionTarget::LocalHost);
    assert!(decision.decision_reason.contains("thermal throttling"));
}

#[test]
fn test_scheduler_host_load_compensation() {
    // Simulate host CPU being 85% busy (host load factor = 0.15)
    let config = SchedulerConfig {
        host_load_factor: 0.15,
        ..Default::default()
    };
    let scheduler = Scheduler::new(config, CostModel::default());
    let worker_profile = create_mock_worker_profile(Some(90), Some(true), Some("none".into()));

    let decision = scheduler.evaluate_task(
        2_000_000,
        50,
        64 * 1024,
        64 * 1024,
        Some(&worker_profile),
        Some(2.0),
    );

    assert!(decision.constraints.all_passed());
    // Under heavy host load, offloading to phone is predicted to be faster!
    assert!(decision.approved_for_offload);
    assert_eq!(decision.target, ExecutionTarget::RemoteWorker);
    assert!(decision.decision_reason.contains("Offload to phone worker"));
}

#[test]
fn test_scheduler_link_latency_rejection() {
    let scheduler = Scheduler::default();
    let worker_profile = create_mock_worker_profile(Some(90), Some(true), Some("none".into()));

    // Excessive RTT: 450 ms (threshold is 250 ms)
    let decision = scheduler.evaluate_task(
        500_000,
        50,
        64 * 1024,
        64 * 1024,
        Some(&worker_profile),
        Some(450.0),
    );

    assert!(!decision.constraints.link_ok);
    assert!(!decision.approved_for_offload);
    assert!(decision.decision_reason.contains("ADB link latency"));
}
