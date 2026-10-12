use anyhow::{Context, Result};
use pocketbridge_core::{
    read_envelope, write_envelope, AdbTransport, AgentRole, Envelope, HeartbeatPayload, HelloPayload,
    MessageKind,
};

pub async fn run(port: u16) -> Result<()> {
    println!("Connecting to PocketBridge worker on port {port}...");
    let transport = AdbTransport::new(port);

    // 1. Device check
    let device = transport.check_device().await
        .context("ADB device check failed")?;
    println!("ADB device detected: {device}");

    // 2. Setup forwarding
    transport.setup_forwarding().await
        .context("Failed to set up ADB port forwarding")?;
    println!("ADB forwarding established: 127.0.0.1:{port} -> device:{port}");

    // 3. Connect TCP
    println!("Connecting to worker at 127.0.0.1:{port}...");
    let mut stream = transport.connect_tcp().await
        .context("TCP connection to worker failed. Is the worker app running and service started?")?;
    println!("Connected successfully!");

    // 4. Send HELLO
    let hello_env = Envelope::hello("pocketbridge-cli", "0.1.0", AgentRole::Host);
    write_envelope(&mut stream, &hello_env).await
        .context("Failed to send HELLO envelope")?;
    println!("Sent HELLO handshake...");

    // 5. Read HELLO reply
    let reply = read_envelope(&mut stream).await
        .context("Failed to receive HELLO response")?;
    if reply.kind != MessageKind::Hello {
        anyhow::bail!("Expected HELLO response, got {:?}", reply.kind);
    }
    let worker_hello: HelloPayload = serde_json::from_value(reply.payload)
        .context("Failed to parse HELLO payload")?;
    println!(
        "Worker handshook: {} (v{}) [role: {}]",
        worker_hello.agent_name, worker_hello.agent_version, worker_hello.role
    );

    // 6. Send HEARTBEAT
    let hb = Envelope::heartbeat(HeartbeatPayload {
        uptime_ms: 0,
        cpu_usage_percent: None,
        memory_used_bytes: None,
        memory_total_bytes: None,
        temperature_celsius: None,
        battery_percent: None,
        battery_charging: None,
        thermal_status: None,
    });
    write_envelope(&mut stream, &hb).await
        .context("Failed to send HEARTBEAT")?;

    let hb_reply = read_envelope(&mut stream).await
        .context("Failed to receive HEARTBEAT reply")?;
    if hb_reply.kind == MessageKind::Heartbeat {
        let worker_hb: HeartbeatPayload = serde_json::from_value(hb_reply.payload)?;
        println!(
            "Worker heartbeat reply: uptime={}s, battery={:?}%, charging={:?}, temp={:?}C, thermal={:?}",
            worker_hb.uptime_ms / 1000,
            worker_hb.battery_percent,
            worker_hb.battery_charging,
            worker_hb.temperature_celsius,
            worker_hb.thermal_status
        );
    }

    println!("\nConnection verification complete. Worker is active and responsive!");
    Ok(())
}
