use anyhow::Result;
use pocketbridge_core::{
    AdbTransport, AgentRole, CostModel, DeviceProfile, Envelope, MessageKind,
    Scheduler, SchedulerConfig, read_envelope, write_envelope,
    profiler::build_worker_profile_from_props,
};

pub async fn run(
    records: u32,
    rounds: u32,
    host_load: f64,
    margin: f64,
    port: u16,
) -> Result<()> {
    println!("PocketBridge Explainable Scheduler — Task Planning & Decision Analysis");
    println!("==========================================================================");
    println!("Target Workload:     {} records, {} rounds per record", records, rounds);
    println!("Simulated Host Load: {:.0}% available capacity (factor: {:.2})", host_load * 100.0, host_load);
    println!("Required Margin:     {:.2}x speedup threshold\n", margin);

    let config = SchedulerConfig {
        speedup_margin: margin,
        host_load_factor: host_load,
        ..Default::default()
    };
    let scheduler = Scheduler::new(config, CostModel::default());

    // Check if phone worker is accessible over ADB to query live battery & thermal status
    let transport = AdbTransport::new(port);
    let mut worker_profile: Option<DeviceProfile> = None;
    let mut rtt_ms: Option<f64> = None;

    if transport.check_device().await.is_ok() {
        if let Ok(props) = transport.query_getprop().await {
            let mut profile = build_worker_profile_from_props(&props);
            if transport.setup_forwarding().await.is_ok() {
                if let Ok(mut stream) = transport.connect_tcp().await {
                    let hello = Envelope::hello("pocketbridge-plan", "0.1.0", AgentRole::Host);
                    let _ = write_envelope(&mut stream, &hello).await;
                    if let Ok(reply) = read_envelope(&mut stream).await {
                        if reply.kind == MessageKind::Hello {
                            // Ping for live RTT
                            let t_ping = std::time::Instant::now();
                            let ping_env = Envelope::benchmark_echo(1, 0, pocketbridge_core::protocol::current_unix_ms());
                            let _ = write_envelope(&mut stream, &ping_env).await;
                            if let Ok(ping_reply) = read_envelope(&mut stream).await {
                                if ping_reply.kind == MessageKind::BenchmarkEchoReply {
                                    rtt_ms = Some(t_ping.elapsed().as_secs_f64() * 1000.0);
                                }
                            }

                            // Query live heartbeat telemetry (battery, charging, thermal, RAM)
                            let hb = Envelope::heartbeat(pocketbridge_core::HeartbeatPayload {
                                uptime_ms: 0,
                                cpu_usage_percent: None,
                                memory_used_bytes: None,
                                memory_total_bytes: None,
                                temperature_celsius: None,
                                battery_percent: None,
                                battery_charging: None,
                                thermal_status: None,
                            });
                            let _ = write_envelope(&mut stream, &hb).await;
                            if let Ok(hb_reply) = read_envelope(&mut stream).await {
                                if hb_reply.kind == MessageKind::Heartbeat {
                                    if let Ok(worker_hb) = serde_json::from_value::<pocketbridge_core::HeartbeatPayload>(hb_reply.payload) {
                                        profile.battery_percent = worker_hb.battery_percent;
                                        profile.battery_charging = worker_hb.battery_charging;
                                        profile.thermal_status = worker_hb.thermal_status;
                                        if let (Some(used), Some(total)) = (worker_hb.memory_used_bytes, worker_hb.memory_total_bytes) {
                                            profile.ram_total_bytes = total;
                                            profile.ram_available_bytes = Some(total.saturating_sub(used));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            worker_profile = Some(profile);
        }
    }

    if worker_profile.is_none() {
        println!("Note: No physical phone detected over ADB. Evaluating using Snapdragon 8 Elite hardware baseline.\n");
    }

    let input_bytes = (records as usize) * 8;
    let output_bytes = (records as usize) * 8;

    let decision = scheduler.evaluate_task(
        records,
        rounds,
        input_bytes,
        output_bytes,
        worker_profile.as_ref(),
        rtt_ms,
    );

    let card = Scheduler::format_decision_card(&decision);
    println!("{card}");

    Ok(())
}
