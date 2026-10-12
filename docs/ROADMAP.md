# Roadmap

The roadmap is ordered to reduce uncertainty before building complex integrations. Every phase should end with a demo, test evidence, and a decision to continue, revise, or stop.

## Phase 0 — Feasibility notebook

**Goal:** Establish the real connection and resource constraints before choosing implementation details.

Tasks:
- Record laptop CPU/architecture, RAM, GPU, kernel, USB controller, and Android version.
- Record phone SoC/ABI, RAM, storage, Android thermal and battery behavior relevant to sustained workloads.
- Test ADB authorization and device stability through USB-C.
- Measure small and large request/response latency and throughput.
- Compare USB/ADB with Wi-Fi as a reference, not as an assumption that one is better.
- Decide whether Termux can host the first worker process reliably under the intended screen/charging conditions.

**Exit criteria:** Reproducible link measurements, documented limitations, and a working hello/heartbeat prototype.

## Phase 1 — Vertical slice

**Goal:** One supported example job runs on the phone and returns a validated result.

Tasks:
- Create Rust host CLI and worker client interface.
- Create the smallest worker process that can answer `HELLO`, `CAPABILITIES`, and `HEARTBEAT`.
- Add framed messages, timeouts, and protocol-version checking.
- Add one deterministic compute task with known expected output.
- Run locally and remotely; compare results.
- Record end-to-end time including transfers and startup.

**Exit criteria:** Repeatable successful jobs, no silent result corruption, and a visible latency/speedup report.

## Phase 2 — Job lifecycle and reliability

**Goal:** Handle ordinary failures without corrupting state.

Tasks:
- Persist job state and attempt IDs.
- Implement bounded chunked transfer and hash checking.
- Implement cancellation, deadlines, and structured errors.
- Test cable removal during every lifecycle phase.
- Add worker restart/reconnect reconciliation.
- Implement safe retry rules for idempotent jobs only.

**Exit criteria:** Automated failure-injection tests pass and all lost/unknown outcomes are represented explicitly.

## Phase 3 — Explainable scheduling

**Goal:** Choose local or remote execution based on eligibility and measured cost.

Tasks:
- Define adapter capability manifest.
- Estimate transfer and execution cost using benchmark history.
- Add load, free-memory-budget, thermal, and battery constraints where data is reliably available.
- Add policy margin to avoid offloading when estimates are too close or uncertain.
- Show a human-readable decision reason.

**Exit criteria:** Scheduler never submits unsupported tasks and improves end-to-end time for at least one supported workload on target hardware, or documents why it cannot.

## Phase 4 — Workload adapters

**Goal:** Expand useful coverage without pretending to support arbitrary applications.

Candidate order:
1. Independent deterministic data-processing chunks.
2. Image conversion/resize batch where inputs are independent and the worker has a compatible toolchain.
3. Explicit scripting adapter with strong time, input, output, and permission limits.
4. Investigate distributed compilation only after cross-compilation, compiler versions, target triples, and ABI constraints are documented.
5. Investigate inference backends as their own adapter.

Each adapter requires a benchmark, correctness test, security review, and clear compatibility matrix.

## Phase 5 — Native Android worker

**Goal:** Replace prototype dependencies where Android platform integration is needed.

Tasks:
- Decide on a foreground-service lifecycle and user-visible controls.
- Use platform-supported APIs for thermal/battery/availability metadata.
- Package the agent and supported worker runtimes.
- Implement secure pairing/revocation and job cleanup.
- Test screen-off, Doze/background restrictions, low battery, heat, and system process termination.

**Exit criteria:** Worker behavior is understandable and reliable under Android lifecycle limits.

## Phase 6 — Broader integration

**Goal:** Integrate a small number of real user workloads and decide which are worth maintaining.

Tasks:
- Publish an adapter API and versioning policy.
- Add a local status UI only if the CLI workflow is stable.
- Add multi-worker scheduling only after one-worker fault handling works.
- Explore workload-specific app hooks and runtime integrations.
- Keep unsupported operations local with transparent user feedback.

## Research track — Transparent application acceleration

This is deliberately separate from the MVP. Explore one target at a time:

- Can a particular application expose independent worker jobs?
- Can a runtime split work across x86-64 and ARM64 while preserving ABI/semantics?
- Could an application-managed cache or object store reduce memory pressure?
- Is remote memory useful at measured USB/ADB latency, or does it harm responsiveness?
- Can any compute API run on the Android GPU/NPU and return results efficiently?
- For gaming, is the bottleneck actually CPU, GPU, RAM capacity, shader compilation, I/O, or thermal throttling?

**Do not state “works for all games/apps” unless there is a general mechanism and a reproducible compatibility test suite proving it.**

## Release gates

A release must not claim speedup unless the benchmark includes setup and transfer overhead. Security issues that permit arbitrary remote code execution outside the configured sandbox block release. A worker that can be disconnected cleanly is a minimum reliability requirement, not an optional enhancement.
