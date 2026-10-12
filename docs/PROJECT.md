# Project Specification

## 1. Summary

PocketBridge is an exploratory distributed-computing system intended to let a Linux laptop use spare compute capacity from a connected Android phone. Its near-term purpose is to offload supported work to a worker process running on the phone. Its long-term ambition is to make resource sharing more automatic and useful across more applications.

The project's motivating scenario is a laptop under resource pressure while a phone is available over USB-C. The system should determine whether some eligible work can run on the phone and whether moving that work is likely to improve the result.

## 2. Problem statement

A conventional laptop cannot directly add an Android phone's physical RAM, CPU cores, or GPU to its own hardware inventory. Processes assume local address spaces, operating-system services, executable ABIs, and device drivers. Moving work to a second device therefore requires a defined execution boundary.

PocketBridge addresses the subset of work that can cross that boundary. It will progressively investigate better integration, but must not confuse remote task execution with transparent hardware pooling.

## 3. Goals

- Use an attached Android phone as an optional secondary worker.
- Prefer USB-C in the first prototype while keeping the transport replaceable.
- Advertise worker capabilities and current willingness to accept work.
- Define a job contract with inputs, resource requirements, execution limits, outputs, deadline, and retry policy.
- Schedule only tasks that an installed adapter declares safe and compatible.
- Keep local execution available and compare local and remote costs.
- Recover from interruptions without corrupting host state or silently losing results.
- Collect evidence about latency, throughput, energy, thermal throttling, memory pressure, and net benefit.
- Make the architecture extensible to other Android devices and, later, additional workers.

## 4. Non-goals

- Universal migration of arbitrary running processes.
- A shared physical memory address space between laptop and phone.
- Transparent RAM expansion for games.
- A drop-in replacement for a laptop GPU or its graphics driver.
- A promise that USB-C will be faster than Wi-Fi for every transfer or workload.
- Mandatory root access or a modified Android kernel.
- Running code from unknown users or external networks.

## 5. Users and core scenarios

### Scenario A — supported batch task

A user launches a supported batch job. The host estimates input-transfer and execution cost, runs locally or remotely, validates the output, and reports the comparison.

### Scenario B — laptop under load

The host detects high CPU or memory pressure. It considers only workloads with a supported adapter and enough separability. It must not assume that all running work can be offloaded.

### Scenario C — worker becomes unavailable

USB is unplugged, Android suspends the worker, or the device refuses more work. The host cancels or retries only idempotent jobs, cleans up temporary data, and preserves a clear job state.

### Scenario D — user opts into a workload

The user selects an application adapter and approves the data classes and resource limits it may send to the phone. No automatic process-memory inspection or bulk file upload is enabled by default.

## 6. Terminology

- **Host:** Linux laptop that owns the user session and scheduling policy.
- **Worker:** Android phone process that executes a compatible job.
- **Adapter:** Integration that converts a supported workload into a job request and consumes its result.
- **Job:** Bounded unit of work with a defined input and output contract.
- **Offloading:** Running a job on another device instead of locally.
- **Transparent acceleration:** Making an existing application benefit without explicit workload adaptation. This is research scope, not an MVP capability.
- **Remote memory:** Memory capacity reached over a transport. It is not local DRAM and has transfer latency and failure modes.

## 7. Product requirements

### Functional

- FR-01: Discover the connected worker after the user authorizes it.
- FR-02: Pair the host with a specific worker and store a revocable identity.
- FR-03: Receive worker metadata: OS/API level, ABI, supported runtimes, memory budget, compute capabilities, power state if available, and agent version.
- FR-04: Submit, query, cancel, and collect jobs.
- FR-05: Apply per-job CPU, memory, wall-time, data-size, and output-size limits.
- FR-06: Record job lifecycle states and timestamps.
- FR-07: Reject work with incompatible ABI, runtime, security, deadline, or resource requirements.
- FR-08: Compare local and remote execution for benchmarkable jobs.
- FR-09: Handle duplicate messages and interrupted transfers safely.
- FR-10: Explain scheduler decisions to the user.
- FR-11: Allow the user to disable the worker immediately.
- FR-12: Avoid automatic retry unless the job is declared idempotent or has a deduplication key.

### Non-functional

- NFR-01: Secure-by-default local pairing and authenticated protocol.
- NFR-02: No public network listener by default.
- NFR-03: Bounded resource use on both devices.
- NFR-04: Observable behavior: logs, metrics, job states, and failure reasons.
- NFR-05: Reproducible benchmark inputs and recorded device/software configuration.
- NFR-06: Modular transports and workload adapters.
- NFR-07: Clear degradation when the phone is hot, busy, charging poorly, or disconnected.
- NFR-08: No claim of universal acceleration without independent tests per workload.

## 8. Definition of MVP

The MVP is a command-line prototype, not a transparent operating-system layer. It should:

1. Connect one Arch Linux host to one Android worker over a user-authorized USB/ADB channel.
2. Run a deterministic example workload locally and remotely.
3. Validate that both outputs are equivalent.
4. Measure startup overhead, input transfer, execution, output transfer, and total elapsed time.
5. Enforce job limits and refuse unsupported jobs.
6. Demonstrate safe behavior after cable removal or worker termination.
7. Provide a simple summary showing local time, remote time, speedup or slowdown, bytes transferred, and failure state.

### Candidate proof workload

Use a deterministic, embarrassingly parallel task such as processing independent chunks of generated numeric data or applying a CPU-bound transform to many independent records. Do not start with a running game or arbitrary process. The task should be large enough that computation might amortize transfer and startup overhead, but small enough to repeat reliably.

## 9. Success metrics

For each job, record:

- Local wall-clock duration.
- Remote wall-clock duration including startup and transfer.
- Input and output bytes.
- Host and worker CPU time where available.
- Peak worker memory estimate where available.
- Queue time, transfer time, execution time, and result-validation time.
- Worker temperature/thermal status when safely available.
- Battery/charging state when safely available.
- Correctness result.
- Recovery result after controlled disconnection.

Define speedup as `local_elapsed / remote_total_elapsed`. Values greater than 1 indicate faster remote execution; values below 1 indicate slower remote execution. Do not omit transfer and setup overhead when reporting end-to-end speedup.

## 10. Constraints and assumptions

- First host is Arch Linux on x86-64.
- Initial worker is Android ARM64.
- ADB over USB is the first transport experiment; whether the chosen Android setup supports the intended persistent service model must be validated.
- Android may restrict background execution, networking, thermal headroom, and long-running jobs. The worker must account for those restrictions.
- Host and phone do not share a physical memory bus or native process address space.
- A remote worker can execute only code and operations supported by its OS, architecture, runtime, permissions, and available APIs.
- A USB-C cable's connector shape does not alone specify negotiated speed, supported modes, or available protocol features.

## 11. User experience principles

- Show what is being offloaded and why.
- Show estimated benefit with uncertainty, not just a green “accelerated” label.
- Explain when an operation stays local because remote execution would likely be slower or unsafe.
- Use clear controls: connect, trust, pause, revoke, inspect queue, cancel job, and clear temporary data.
- Keep verbose diagnostics available without forcing ordinary users to manage low-level transport details.
