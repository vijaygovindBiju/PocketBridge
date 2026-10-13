# Architecture Decision Records

These records document architectural and design decisions, trade-offs, and historical rationale across the evolution of PocketBridge.

## ADR-001 — Start with a host coordinator and phone worker

**Status:** Accepted for prototype

**Decision:** The laptop remains the host and policy authority; the phone runs a worker agent.

**Reasoning:** This is testable on ordinary devices and creates a clear boundary for task distribution. It does not pretend to merge the operating systems or process address spaces.

**Consequence:** Work must be represented as supported jobs. Arbitrary running processes are not automatically migratable.

---

## ADR-002 — Rust for the Linux daemon

**Status:** Accepted

**Decision:** Use Rust for host orchestration, protocol handling, job lifecycle, scheduler, and local CLI.

**Reasoning:** Systems-oriented language with explicit control over concurrency, process management, and native performance (AVX2/SHA-NI).

**Consequence:** Keep Android-specific lifecycle concerns behind the worker boundary; use Kotlin for Android platform integration and native C++ for performance-critical kernels.

---

## ADR-003 — Strictly Wired USB-to-USB Link via ADB Forwarding

**Status:** Accepted

**Decision:** Use an ADB-forwarded local TCP channel (`127.0.0.1:9876`) over a physical USB-C cable.

**Reasoning:** Physical USB-C delivers sub-millisecond RTT (~0.18 ms) and > 125 MB/s throughput without exposing any Wi-Fi/LAN attack surface. ADB port forwarding operates seamlessly on non-rooted Android devices.

**Consequence:** Requires developer options and USB debugging enabled on the worker phone. Wi-Fi transport is strictly excluded.

---

## ADR-004 — Explicit job contracts before transparent acceleration

**Status:** Accepted

**Decision:** Only offload jobs declared compatible by an adapter.

**Reasoning:** Arbitrary process migration across Linux x86-64 and Android ARM64 without explicit schemas leads to undefined behavior.

**Consequence:** Initial adapters focus on structured data, image batching, and stream processing.

---

## ADR-005 — No remote-memory pooling in MVP

**Status:** Accepted

**Decision:** Do not expose phone storage/RAM as a Linux swap device or advertise it as added host RAM.

**Reasoning:** Remote page access has latency and transport failure costs; general game/process memory semantics are not solved by a simple worker agent.

**Consequence:** Memory-related experiments remain in the post-v1.0 research track.

---

## ADR-006 — Benchmark end-to-end performance

**Status:** Accepted

**Decision:** All public speedup claims include startup, queueing, input transfer, remote execution, output transfer, and validation.

**Reasoning:** Compute-only measurements hide real-world transport overheads.

**Consequence:** Workloads where transfer overhead exceeds compute gain are rejected by the scheduler.

---

## ADR-007 — Dynamic Capability-Based Workload Partitioning

**Status:** Accepted (Phase 4 / Commit `3c7aea6`)

**Decision:** Auto-calibrate the workload division ratio between host and phone using live hardware profiles (CPU cores, clock frequency, thermal status) rather than fixed 50/50 splits.

**Reasoning:** A flagship phone (Snapdragon 8 Elite @ 4.32 GHz) outperforms an idle mid-range laptop core, but heavy phone thermal throttling reduces capacity. Live profiling ensures optimal load distribution.

**Consequence:** Enabled `--hybrid` co-processing with automatic dynamic ratio calculation.

---

## ADR-008 — Universal Process Partitioning via Stream Slicing

**Status:** Accepted (Phase 7 / Commit `bd38c41`)

**Decision:** Implement `pocketbridge exec` to partition arbitrary Unix command line streams across laptop and phone using line-by-line, byte-chunk, or itemized slicing.

**Reasoning:** Enables users to accelerate arbitrary CLI commands (e.g. `tr`, `grep`, `sha256sum`, data transformation scripts) without writing dedicated adapters.

**Consequence:** Stream partitioner must accurately reconstruct output ordering.

---

## ADR-009 — AI Speculative Decoding & Layer Splitting over USB

**Status:** Accepted (Phase 9 / Commit `4b6aae1`)

**Decision:** Partition Transformer LLM layers across host and phone, and run phone NPU speculative drafting alongside host verification passes.

**Reasoning:** Activation tensor vectors (~8 KB per token) transmit in < 0.2 ms over USB-C, making distributed inference viable. Speculative drafting delivers > 3x effective speedup.

**Consequence:** Requires quantized weights on the worker and synchronized tokenizer vocabularies.

---

## ADR-010 — Option 2 Micro-Kernel Architecture & Decoupled Plugin Hub

**Status:** Accepted (Phase 10 / Commit `d3d1665`)

**Decision:**
1. Keep only `chunk_transform.v1` as the sole built-in adapter in core PocketBridge (~50 KB C++ SIMD kernel) for zero-download USB link calibration.
2. Decouple heavy workloads (`image-batch`, `data-analytics`, `ai-whisper`, `browser-cdp`, `media-ffmpeg`) into downloadable plugins.
3. Host the community plugin registry in a separate public repository: [pocketbridge-plugins](https://github.com/vijaygovindBiju/pocketbridge-plugins).

**Reasoning:**
- Core binary remains lightweight (< 4 MB).
- First-time users can immediately test hardware and USB connectivity without downloading anything.
- Community contributors can submit new plugins via Pull Request without modifying or jeopardizing core PocketBridge engine code.

**Consequence:** Plugins are installed on-demand via `pocketbridge plugin install <id>` and verified cryptographically with SHA-256 signatures before activation.
