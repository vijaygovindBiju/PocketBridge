# PocketBridge — Experiments & Empirical Benchmarking Methodology

This document outlines the hardware testbed specifications, empirical benchmarking methodologies, instrumentation, and analytical models used to measure physical USB-C co-processing performance between the Linux host and the Android mobile device.

---

## 1. Hardware Testbed Environment

All benchmarks in PocketBridge are designed to execute on physical hardware:

| Component | Hardware Specification | Role & Environment |
|---|---|---|
| **Host System** | Intel Core i5-13420H (8 Cores: 4P+4E, 12 Threads, up to 4.60 GHz), 16GB DDR5 RAM | Arch Linux (Kernel 6.x x86_64), Rust (release profile) |
| **Worker Device** | Samsung Galaxy S25 Ultra (`SM-S938B`), Qualcomm Snapdragon 8 Elite (2 Oryon Prime @ 4.32 GHz + 6 Oryon Performance @ 3.53 GHz), 12GB LPDDR5X RAM | Android 15 (API 35), NDK native SIMD engine |
| **Physical Link** | Anker USB-C 3.2 Gen 1 (5 Gbps physical PHY) | ADB port-forwarded socket (`127.0.0.1:9876` / `127.0.0.1:50052`) |
| **Baseline Link** | Standard USB 2.0 A-to-C cable (480 Mbps physical PHY) | ADB port-forwarded socket (`127.0.0.1:9876`) |

---

## 2. Methodology & Instrumentation

- **Transport:** ADB TCP port forwarding over physical USB-C cable (`adb forward tcp:9876 tcp:9876` and `adb forward tcp:50052 tcp:50052`).
- **Framing:** Length-prefixed binary framing (`0x504B5442`) with constant memory $O(1)$ chunked streaming (64KB chunks).
- **Execution:** Automated benchmark suites executed via `pocketbridge bench --runs 25` and `pocketbridge run --hybrid`, writing machine-readable JSON metrics to `benchmarks/results/`.
- **Metrics Collected:**
  - Round-trip time (RTT): `median_ms`, `p95_ms`, `p99_ms`, `min_ms`, `max_ms`, `stddev_ms`.
  - Effective throughput in Mbps / MB/s across payload sweeps (64 B to 100 MB).
  - Wall-clock compute duration ($T_{\text{wall}}$), host execution time ($T_{\text{host}}$), worker compute time ($T_{\text{worker}}$).
  - Thermal status via `PowerManager.getThermalHeadroom` and battery current/temperature via `dumpsys battery`.
  - Bit-exact result verification via cryptographic SHA-256 digests.

---

## 3. Link Transfer Benchmarking Policy

In strict adherence to engineering honesty:
- **No Mock Results:** Benchmark tables are populated **only** from live hardware runs executed via `pocketbridge bench`.
- **Userspace ADB Tunnel Overhead:** While physical USB 3.2 Gen 1 hardware interfaces support multi-gigabit raw PHY data rates, non-root communication over ADB traverses userspace socket multiplexing on both Linux (`adb server`) and Android (`adbd`). Real-world RTT and throughput must always be gathered from live measurements, not theoretical PHY calculations.
- **Output Storage:** Each run of `pocketbridge bench` records timestamped JSON summaries into `benchmarks/results/`.

---

## 4. Head-to-Head Compute Comparison Methodology

To evaluate co-processing efficiency on real hardware, workloads are evaluated across three configurations:
1. **Laptop-Only Baseline:** Local Intel CPU (multithreaded AVX2).
2. **Worker-Only Offload:** Mobile Snapdragon 8 Elite CPU (Native NDK C++ ARM NEON).
3. **PocketBridge Hybrid Co-Processing:** Dynamically auto-calibrated partition ratio across both devices running concurrently.

### Benchmark Workloads:
- **`chunk_transform.v1`:** Calculates running rolling polynomial hashes, SIMD vector arithmetic, and cryptographic block reductions across $N$ structured records.
- **`data_analytics.v1`:** Aggregates, filters, and computes multi-column statistical quantiles across tabular dataset arrays.
- **`image_batch.v1`:** Applies 2D box-blur convolution and color channel transformation across batches of images.

> **Integrity Rule:** All benchmark speedups must be backed by live execution with matching SHA-256 digests. No estimated or simulated speedup multipliers are published without real device measurements.

---

## 5. Analytical Amortization & Breakeven Model

Offloading to an external co-processor is only beneficial when the remote execution savings exceed the serial transport latency:

$$T_{\text{offload}} = T_{\text{serialize}} + T_{\text{upload}} + T_{\text{worker\_compute}} + T_{\text{download}} + T_{\text{deserialize}}$$

For hybrid concurrent execution partitioned with worker fraction $\alpha \in (0, 1)$:

$$T_{\text{hybrid}} = \max\Big( T_{\text{host}}(1 - \alpha), \; T_{\text{xfer}}(\alpha) + T_{\text{worker\_compute}}(\alpha) \Big) + T_{\text{combine}}$$

### Breakeven Decision Boundary:
- **Trivial tasks ($T_{\text{compute}} \le T_{\text{xfer}}$):** The Policy Engine schedules 100% locally on the host CPU. Moving data over USB produces negative returns (Amdahl's law bottleneck).
- **Partitionable compute-dense tasks ($T_{\text{compute}} \gg T_{\text{xfer}}$):** Concurrent co-processing amortizes the transfer overhead, delivering net speedups.

---

## 6. Thermal, Battery & Stability Profiling Methodology

When connected to physical hardware, long-running stress tests monitor device longevity:
1. **Thermal Headroom:** Continuous polling of `PowerManager.getThermalHeadroom` and `dumpsys battery` temperature. If temperature exceeds $42^\circ\text{C}$, the scheduler ramps down offloading ratios or rejects offloading.
2. **Power Delivery:** USB-C power delivery is tracked via battery level and charging state to ensure device battery equilibrium.
3. **Deterministic Memory Bounds:** Direct byte-buffer memory pools and chunked streaming keep JVM and Rust heap footprints bounded under 64MB regardless of workload duration.
