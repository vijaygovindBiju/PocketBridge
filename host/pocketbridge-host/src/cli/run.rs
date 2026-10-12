use anyhow::{Context, Result};
use pocketbridge_core::{
    AdbTransport, AgentRole, ChunkTransformParams, Envelope, HelloPayload, LocalExecutor,
    MessageKind, RemoteJobClient, read_envelope, write_envelope,
};

pub async fn execute(
    records: u32,
    rounds: u32,
    seed: u64,
    local_only: bool,
    remote_only: bool,
    port: u16,
) -> Result<()> {
    println!("PocketBridge Compute Workload Runner (chunk_transform.v1)");
    println!("==========================================================================");
    println!("Parameters: {} records, {} rounds per record, seed 0x{:016X}", records, rounds, seed);
    let params = ChunkTransformParams {
        record_count: records,
        rounds,
        seed,
    };

    // 1. Local Run
    let local_output = if !remote_only {
        println!("\n>> Running locally on Arch Linux host...");
        let out = LocalExecutor::run_transform(&params);
        println!("   Local compute duration: {:.2} ms", out.duration_ms);
        println!("   Local output SHA-256:   {}", out.output_sha256);
        Some(out)
    } else {
        None
    };

    if local_only {
        println!("\nCompleted (local-only mode).");
        return Ok(());
    }

    // 2. Remote Run via Worker
    println!("\n>> Preparing remote execution on phone...");
    let transport = AdbTransport::new(port);
    let device = transport.check_device().await
        .context("ADB device check failed. Is phone connected via USB debugging?")?;
    println!("   Target device: {device}");

    transport.setup_forwarding().await
        .context("Failed to setup ADB port forwarding")?;

    println!("   Connecting to worker at 127.0.0.1:{port}...");
    let mut stream = transport.connect_tcp().await
        .context("Failed to connect to worker service")?;

    // Handshake
    let hello = Envelope::hello("pocketbridge-run", "0.1.0", AgentRole::Host);
    write_envelope(&mut stream, &hello).await?;
    let reply = read_envelope(&mut stream).await?;
    if reply.kind != MessageKind::Hello {
        anyhow::bail!("Expected HELLO response, got {:?}", reply.kind);
    }
    let worker_hello: HelloPayload = serde_json::from_value(reply.payload)?;
    println!("   Handshook with worker: {} (v{})\n", worker_hello.agent_name, worker_hello.agent_version);

    // Execute remote job
    println!(">> Submitting and monitoring remote job on phone...");
    let result = RemoteJobClient::execute_remote_job(
        &mut stream,
        &params,
        local_output.as_ref(),
        |fraction, msg| {
            print!("\r   Progress: [{:>3.0}%] {}", fraction * 100.0, msg);
            use std::io::Write;
            let _ = std::io::stdout().flush();
        },
    ).await.context("Failed executing remote job")?;

    println!("\n\n==========================================================================");
    println!("Job Execution Summary");
    println!("==========================================================================");
    println!("Job ID:               {}", result.job_id);
    println!("Final Status:         {}", result.state);
    println!("Records Processed:    {}", result.records_processed);

    if let Some(local_ms) = result.local_duration_ms {
        println!("Local Compute Time:   {:.2} ms", local_ms);
        println!("Local SHA-256:        {}", result.local_sha256.as_deref().unwrap_or("-"));
    }

    if let Some(remote_comp) = result.remote_compute_ms {
        println!("Phone Compute Time:   {:.2} ms", remote_comp);
    }
    if let Some(up) = result.remote_upload_ms {
        println!("Network Upload Time:  {:.2} ms", up);
    }
    if let Some(dl) = result.remote_download_ms {
        println!("Network Download Time:{:.2} ms", dl);
    }
    if let Some(total_rem) = result.remote_total_ms {
        println!("Total Remote Elapsed: {:.2} ms (including network transfer & startup)", total_rem);
    }
    if let Some(sha) = &result.remote_sha256 {
        println!("Phone SHA-256:        {}", sha);
    }

    println!("--------------------------------------------------------------------------");
    if result.outputs_match {
        println!("Result Equivalence:   VERIFIED (Both outputs match bit-for-bit!)");
    } else {
        eprintln!("Result Equivalence:   MISMATCH!");
    }

    if let Some(speedup) = result.speedup {
        if speedup > 1.0 {
            println!("End-to-End Speedup:   {:.2}x FASTER remotely (Speedup > 1.0)", speedup);
        } else {
            println!("End-to-End Speedup:   {:.2}x (Local was faster due to transfer/startup overhead)", speedup);
        }
    }
    println!("==========================================================================");

    Ok(())
}
