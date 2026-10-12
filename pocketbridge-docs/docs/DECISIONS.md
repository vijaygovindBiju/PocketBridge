# Architecture Decision Records

These are initial decisions, not permanent truths. Revisit them when benchmark evidence contradicts the assumptions.

## ADR-001 — Start with a host coordinator and phone worker

**Status:** Accepted for prototype

**Decision:** The laptop remains the host and policy authority; the phone runs a worker agent.

**Reasoning:** This is testable on ordinary devices and creates a clear boundary for task distribution. It does not pretend to merge the operating systems or process address spaces.

**Consequence:** Work must be represented as supported jobs. Arbitrary running processes are not automatically migratable.

## ADR-002 — Rust for the Linux daemon

**Status:** Proposed

**Decision:** Use Rust for host orchestration, protocol handling, job lifecycle, scheduler, and local CLI.

**Reasoning:** Good fit for a systems-oriented Linux project and explicit control over concurrency and process management.

**Consequence:** Keep Android-specific lifecycle concerns behind the worker boundary; do not force every Android component into Rust if Kotlin/platform APIs are more appropriate.

## ADR-003 — ADB over USB as the first transport experiment

**Status:** Proposed; requires validation

**Decision:** Begin with an ADB-forwarded local TCP channel while keeping transport pluggable.

**Reasoning:** Official ADB supports port forwarding and allows a prototype without requiring a custom USB network driver.

**Consequence:** Requires user-authorized USB debugging and may be unsuitable for a polished consumer setup. Measure throughput and latency; do not assume it is equivalent to raw USB.

## ADR-004 — Explicit job contracts before transparent acceleration

**Status:** Accepted

**Decision:** Only offload jobs declared compatible by an adapter.

**Reasoning:** Arbitrary process migration, transparent shared RAM, and GPU pooling across Linux x86-64 and Android ARM64 are much harder than task RPC.

**Consequence:** Initial support will be narrower than the ultimate vision, but correctness and measurable performance will be possible.

## ADR-005 — No remote-memory pooling in MVP

**Status:** Accepted

**Decision:** Do not expose phone storage/RAM as a Linux swap device or advertise it as added host RAM.

**Reasoning:** Remote page access has latency and transport failure costs; general game/process memory semantics are not solved by a simple worker agent.

**Consequence:** Memory-related experiments remain a separate research track and must be compared against local zram/swap baselines.

## ADR-006 — Benchmark end-to-end performance

**Status:** Accepted

**Decision:** All public speedup claims include startup, queueing, input transfer, remote execution, output transfer, and validation.

**Reasoning:** Compute-only measurements can hide the overhead that determines whether offloading actually helps the user.

**Consequence:** Some workloads will be rejected by the scheduler or explicitly shown as slower remotely.
