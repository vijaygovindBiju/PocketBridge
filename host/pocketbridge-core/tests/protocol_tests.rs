use pocketbridge_core::{
    calculate_stats, drain_raw_stream, read_envelope, run_single_echo, write_envelope,
    write_raw_stream, AgentRole, BenchmarkEchoPayload,
    Envelope, HeartbeatPayload, HelloPayload, MessageKind, SampleResult,
};
use tokio::net::TcpListener;

#[test]
fn test_envelope_serialization() {
    let env = Envelope::hello("test-agent", "0.1.0", AgentRole::Host).with_priority(3);
    let json = serde_json::to_string(&env).expect("Serialize to JSON");

    let deserialized: Envelope = serde_json::from_str(&json).expect("Deserialize from JSON");
    assert_eq!(deserialized.kind, MessageKind::Hello);
    assert_eq!(deserialized.priority, Some(3));
    assert_eq!(deserialized.protocol_version, 1);

    let hello_payload: HelloPayload = serde_json::from_value(deserialized.payload).unwrap();
    assert_eq!(hello_payload.agent_name, "test-agent");
    assert_eq!(hello_payload.agent_version, "0.1.0");
    assert_eq!(hello_payload.role, AgentRole::Host);
}

#[test]
fn test_heartbeat_payload() {
    let hb = HeartbeatPayload {
        uptime_ms: 120_000,
        cpu_usage_percent: Some(15.5),
        memory_used_bytes: Some(4 * 1024 * 1024 * 1024),
        memory_total_bytes: Some(16 * 1024 * 1024 * 1024),
        temperature_celsius: Some(36.2),
        battery_percent: Some(85),
        battery_charging: Some(true),
        thermal_status: Some("none".into()),
    };
    let env = Envelope::heartbeat(hb);
    let json = serde_json::to_string(&env).unwrap();
    let parsed: Envelope = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.kind, MessageKind::Heartbeat);

    let parsed_hb: HeartbeatPayload = serde_json::from_value(parsed.payload).unwrap();
    assert_eq!(parsed_hb.uptime_ms, 120_000);
    assert_eq!(parsed_hb.battery_percent, Some(85));
    assert_eq!(parsed_hb.thermal_status, Some("none".into()));
}

#[test]
fn test_stats_calculation() {
    let samples = vec![
        SampleResult { sequence: 1, round_trip_ms: 10.0, worker_processing_ms: 1.0, payload_size_bytes: 1000 },
        SampleResult { sequence: 2, round_trip_ms: 20.0, worker_processing_ms: 1.0, payload_size_bytes: 1000 },
        SampleResult { sequence: 3, round_trip_ms: 30.0, worker_processing_ms: 1.0, payload_size_bytes: 1000 },
    ];

    let stats = calculate_stats(&samples, 1000, true);
    assert_eq!(stats.min_ms, 10.0);
    assert_eq!(stats.median_ms, 20.0);
    assert_eq!(stats.max_ms, 30.0);
    assert!(stats.throughput_mbps.is_some());
}

#[tokio::test]
async fn test_mock_worker_tcp_integration() {
    // Spin up mock worker TCP server
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    // Spawn mock worker task
    let worker_handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("worker accepts connection");

        // 1. Receive HELLO
        let req = read_envelope(&mut socket).await.expect("read hello");
        assert_eq!(req.kind, MessageKind::Hello);

        // Reply with worker HELLO
        let reply = Envelope::hello("mock-android-worker", "0.1.0", AgentRole::Worker);
        write_envelope(&mut socket, &reply).await.expect("send hello reply");

        // 2. Receive HEARTBEAT
        let hb_req = read_envelope(&mut socket).await.expect("read heartbeat");
        assert_eq!(hb_req.kind, MessageKind::Heartbeat);

        let hb_reply = Envelope::heartbeat(HeartbeatPayload {
            uptime_ms: 45_000,
            cpu_usage_percent: Some(5.0),
            memory_used_bytes: Some(1024 * 1024 * 1024),
            memory_total_bytes: Some(12 * 1024 * 1024 * 1024),
            temperature_celsius: Some(33.0),
            battery_percent: Some(92),
            battery_charging: Some(true),
            thermal_status: Some("none".into()),
        });
        write_envelope(&mut socket, &hb_reply).await.expect("send heartbeat reply");

        // 3. Receive BENCHMARK_ECHO
        let echo_env = read_envelope(&mut socket).await.expect("read echo env");
        assert_eq!(echo_env.kind, MessageKind::BenchmarkEcho);
        let echo_payload: BenchmarkEchoPayload = serde_json::from_value(echo_env.payload).unwrap();

        // Drain incoming raw bytes
        if echo_payload.binary_size_bytes > 0 {
            drain_raw_stream(&mut socket, echo_payload.binary_size_bytes).await.expect("drain bytes");
        }

        // Send BENCHMARK_ECHO_REPLY
        let now_ms = pocketbridge_core::protocol::current_unix_ms();
        let reply_env = Envelope::benchmark_echo_reply(
            echo_payload.sequence,
            echo_payload.binary_size_bytes,
            echo_payload.sent_at_unix_ms,
            now_ms,
            now_ms + 1,
        );
        write_envelope(&mut socket, &reply_env).await.expect("send echo reply env");

        // Echo raw bytes back
        if echo_payload.binary_size_bytes > 0 {
            write_raw_stream(&mut socket, echo_payload.binary_size_bytes).await.expect("echo raw stream");
        }
    });

    // Client side: connect to mock worker
    let mut client_stream = tokio::net::TcpStream::connect(addr).await.expect("client connects");

    // 1. Handshake
    let client_hello = Envelope::hello("test-host", "0.1.0", AgentRole::Host);
    write_envelope(&mut client_stream, &client_hello).await.expect("client sends hello");

    let worker_reply = read_envelope(&mut client_stream).await.expect("client reads hello reply");
    assert_eq!(worker_reply.kind, MessageKind::Hello);
    let worker_info: HelloPayload = serde_json::from_value(worker_reply.payload).unwrap();
    assert_eq!(worker_info.agent_name, "mock-android-worker");

    // 2. Heartbeat
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
    write_envelope(&mut client_stream, &hb).await.expect("client sends hb");
    let hb_res = read_envelope(&mut client_stream).await.expect("client reads hb reply");
    assert_eq!(hb_res.kind, MessageKind::Heartbeat);

    // 3. Single Echo Benchmark (with 10KB payload)
    let sample = run_single_echo(&mut client_stream, 1, 10_240, Some(2)).await.expect("single echo");
    assert_eq!(sample.sequence, 1);
    assert_eq!(sample.payload_size_bytes, 10_240);
    assert!(sample.round_trip_ms > 0.0);

    worker_handle.await.expect("worker task completed");
}
