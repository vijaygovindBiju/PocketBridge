# Test and Benchmark Plan

## 1. Test layers

### Unit tests

- Protocol envelope validation and version negotiation.
- Job state transition rules.
- Resource limit validation.
- Scheduler decisions for known estimates and compatibility constraints.
- Retry eligibility and idempotency behavior.
- Artifact hashing and manifest validation.
- Temporary-file cleanup and expiry.

### Integration tests

- Host communicates with worker over the selected ADB-forwarded transport.
- Worker reports capabilities and liveness.
- Submit, accept, stream input, execute, stream output, validate, and complete.
- Cancel queued and running jobs.
- Worker restart and host restart recovery.
- Incompatible adapter, ABI, version, or resource request is rejected.

### Fault injection

Disconnect USB during pairing, handshake, input transfer, execution, result transfer, and cancellation. Also test worker process death, host daemon death, timeout, invalid payload, duplicate message, full temporary storage, memory exhaustion, and thermal/battery policy changes where reproducible.

### Device lifecycle tests

- Screen on/off.
- Phone locked/unlocked.
- Charging and not charging.
- Low battery.
- Sustained job and thermal throttling.
- Android background restrictions / worker termination.
- Cable reconnect and authorization reset.

## 2. Benchmark methodology

For each workload:

1. Record hardware, OS/kernel versions, app versions, power mode, battery/charging state, and ambient conditions if available.
2. Validate identical inputs and equivalent outputs.
3. Warm up each path to avoid comparing a cold run to a warm run.
4. Run enough repetitions to report median and spread, not just the best run.
5. Measure the full remote path, including connection/launch overhead and data transfer.
6. Keep local baseline measurements under comparable system load.
7. Report host and worker temperatures/thermal status when available.
8. Report failures and runs excluded from analysis with reasons.
9. Repeat with USB/ADB and Wi-Fi if comparing transports.

## 3. Core metrics

- End-to-end duration.
- Local execution duration.
- Remote execution duration.
- Connection/startup/queue overhead.
- Input and output transfer duration and byte count.
- Throughput and latency distribution.
- CPU utilization and memory consumption where reliably measurable.
- Battery drain/charging state and thermal throttling signals where available.
- Correctness rate and recovery outcome.

Speedup formula:

`speedup = median(local end-to-end duration) / median(remote end-to-end duration)`

Report a speedup only when the comparison is apples-to-apples. A value below 1 means the remote path is slower. Never exclude transfer cost to make the result look better unless clearly presenting a separate compute-only metric alongside the end-to-end result.

## 4. Acceptance criteria for MVP

- 100 repeated example jobs return results matching the local reference.
- Unsupported work is rejected before execution.
- All configured resource limits are enforced or the limitation is documented and the limit omitted from advertised guarantees.
- A disconnected worker never causes the host to mark an unverified job as successful.
- The host and worker can be restarted without corrupting persisted job records.
- No default listener is reachable from an untrusted network.
- Benchmark reports include the full end-to-end cost.
- At least one suitable workload demonstrates a repeatable net benefit, or the project clearly documents that the current link/device/workload combination did not improve performance.

## 5. Performance hypotheses to falsify

- USB/ADB transport overhead is low enough for at least some coarse-grained jobs.
- The phone can sustain useful work without thermal throttling too quickly.
- Enough supported jobs can be identified to make scheduler overhead worthwhile.
- Resource pressure can be relieved by sending independent work away rather than moving a whole running process.

A failed hypothesis is useful evidence. It should narrow the supported workload set, trigger a transport experiment, or prevent a misleading performance claim.
