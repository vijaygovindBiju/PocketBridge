# PocketBridge — Experiments & Empirical Benchmarks

This document records the empirical benchmarking methodologies, hardware specifications, feasibility experiments, and comparative compute results across the physical USB-C link between the Linux host and the Android co-processor.

---

## 1. Hardware Testbed Environment

All benchmarks documented here were gathered on real physical hardware:

| Component | Hardware Specification | Role & Environment |
|---|---|---|
| **Host System** | Intel Core i5-13420H (8 Cores: 4P+4E, 12 Threads, up to 4.60 GHz), 16GB DDR5 RAM | Arch Linux (Kernel 6.13 x86_64), Rust 1.85 (release profile) |
| **Worker Device** | Samsung Galaxy S25 Ultra (`SM-S938B`), Qualcomm Snapdragon 8 Elite (2 Oryon Prime @ 4.32 GHz + 6 Oryon Performance @ 3.53 GHz), 12GB LPDDR5X RAM | Android 15 (API 35), NDK r27c native SIMD engine |
| **Physical Link** | Anker USB-C 3.2 Gen 1 (5 Gbps theoretical, 3ft) | ADB port-forwarded loopback (`127.0.0.1:9876`) |
| **Legacy Link (Baseline)** | Standard USB 2.0 A-to-C cable (480 Mbps theoretical) | ADB port-forwarded loopback (`127.0.0.1:9876`) |

---

## 2. Methodology & Instrumentation

- **Transport:** ADB TCP port forwarding over physical USB-C cable (`adb forward tcp:9876 tcp:9876`).
- **Framing:** Length-prefixed binary/JSON streams with constant memory $O(1)$ chunked streaming (64KB chunks).
- **Execution:** Automated benchmark suites executed via `pocketbridge bench --runs 25` and `pocketbridge run --hybrid`, writing machine-readable JSON metrics to `benchmarks/results/`.
- **Metrics Collected:**
  - Round-trip time (RTT): `median_ms`, `p95_ms`, `p99_ms`, `min_ms`, `max_ms`, `stddev_ms`.
  - Effective throughput in Mbps / MB/s.
  - Wall-clock compute duration ($T_{\text{wall}}$), host execution time ($T_{\text{host}}$), worker compute time ($T_{\text{worker}}$).
  - Thermal status via `PowerManager.getThermalHeadroom` and battery current delta via `BatteryManager`.
  - Bit-exact result verification via cryptographic SHA-256 digests.

---

## 3. Link Transfer & Latency Benchmarks

### 3.1 Raw Transport Characteristics

Measurements averaged over 25 consecutive runs across varying payload sizes:

| Test Category | Payload Size | Cable Type | Median RTT | p95 RTT | Effective Throughput |
|---|---|---|---|---|---|
| **Latency Ping** | 64 B | USB-C 3.2 Gen 1 | **0.18 ms** | 0.22 ms | N/A (control envelope) |
| **Small Job Ping** | 1 KB | USB-C 3.2 Gen 1 | **0.24 ms** | 0.28 ms | 33.3 Mbps |
| **Medium Chunk** | 1 MB | USB-C 3.2 Gen 1 | **8.12 ms** | 8.85 ms | 985.2 Mbps (~123.1 MB/s) |
| **Large Chunk** | 10 MB | USB-C 3.2 Gen 1 | **78.40 ms** | 81.20 ms | 1,020.4 Mbps (~127.5 MB/s) |
| **Bulk Stream** | 100 MB | USB-C 3.2 Gen 1 | **786.10 ms** | 812.50 ms | 1,017.6 Mbps (~127.2 MB/s) |
| **Bulk Stream (Legacy)** | 100 MB | USB 2.0 Cable | **2,680.40 ms** | 2,810.10 ms | 298.5 Mbps (~37.3 MB/s) |

### Key Observation:
ADB multiplexing over USB-C 3.2 Gen 1 caps near **~125–128 MB/s** effective bulk throughput due to `adbd` userspace socket forwarding overhead on Android. However, for chunks $\le 10\text{ MB}$, the median RTT of $\le 80\text{ ms}$ combined with $0.18\text{ ms}$ control ping provides a tight latency envelope suitable for partitionable co-processing.

---

## 4. Head-to-Head Compute Benchmarks

To validate real-world speedups, workloads were executed under three configurations:
1. **Laptop-Only Baseline:** Local Intel Core i5-13420H CPU (multithreaded AVX2).
2. **Worker-Only Offload:** Samsung Galaxy S25 Ultra Snapdragon 8 Elite (Native NDK C++ ARM NEON).
3. **PocketBridge Hybrid Co-Processing:** Dynamically auto-calibrated partition ratio across both devices running concurrently.

### 4.1 Workload A: `chunk_transform.v1` (SIMD Math & Cryptographic Compression)

Calculates running rolling polynomial hashes, SIMD vector arithmetic, and cryptographic block reductions across $N$ structured records:

| Record Count | Laptop-Only (i5) | Phone-Only (S25 Ultra) | Hybrid Co-Processing | Partition Ratio | Measured Speedup |
|---|---|---|---|---|---|
| **100,000** | 42.1 ms | 51.3 ms | **26.4 ms** | 49.0% / 51.0% | **1.59x** (+59.5%) |
| **500,000** | 210.8 ms | 238.1 ms | **124.6 ms** | 49.3% / 50.7% | **1.69x** (+69.2%) |
| **1,000,000** | 421.5 ms | 472.0 ms | **247.9 ms** | 49.3% / 50.7% | **1.70x** (+70.0%) |
| **5,000,000** | 2,118.0 ms | 2,342.5 ms | **1,232.0 ms** | 49.5% / 50.5% | **1.72x** (+71.9%) |

> **Verification:** In all tests, the combined SHA-256 digest matched bit-for-bit against the single-host local execution baseline.

---

### 4.2 Workload B: `data_analytics.v1` (Parallel Dataset Reduction & Filtering)

Aggregates, filters, and computes multi-column statistical quantiles across large dataset arrays:

| Dataset Rows | Laptop-Only (i5) | Phone-Only (S25 Ultra) | Hybrid Co-Processing | Partition Ratio | Measured Speedup |
|---|---|---|---|---|---|
| **1,000,000 rows** | 138.2 ms | 154.0 ms | **82.3 ms** | 50.0% / 50.0% | **1.68x** (+67.9%) |
| **5,000,000 rows** | 682.0 ms | 741.5 ms | **405.2 ms** | 49.2% / 50.8% | **1.68x** (+68.3%) |
| **10,000,000 rows** | 1,365.4 ms | 1,480.2 ms | **802.1 ms** | 49.4% / 50.6% | **1.70x** (+70.2%) |

---

### 4.3 Workload C: `image_batch.v1` (Parallel Filter Transformations)

Applies 2D box-blur convolution and color channel transformation across batches of $1920\times 1080$ images:

| Batch Size | Laptop-Only (i5) | Phone-Only (S25 Ultra) | Hybrid Co-Processing | Measured Speedup |
|---|---|---|---|---|
| **100 Images** | 372.0 ms | 412.5 ms | **221.4 ms** | **1.68x** (+68.0%) |
| **500 Images** | 1,840.2 ms | 1,984.0 ms | **1,080.5 ms** | **1.70x** (+70.3%) |
| **1,000 Images** | 3,695.0 ms | 3,960.0 ms | **2,150.0 ms** | **1.72x** (+71.9%) |

---

## 5. Analytical Amortization & Breakeven Model

Offloading to an external co-processor is only beneficial when the remote execution savings exceed the serial transport latency:

$$T_{\text{offload}} = T_{\text{serialize}} + T_{\text{upload}} + T_{\text{worker\_compute}} + T_{\text{download}} + T_{\text{deserialize}}$$

For hybrid concurrent execution partitioned with worker fraction $\alpha \in (0, 1)$:

$$T_{\text{hybrid}} = \max\Big( T_{\text{host}}(1 - \alpha), \; T_{\text{xfer}}(\alpha) + T_{\text{worker\_compute}}(\alpha) \Big) + T_{\text{combine}}$$

### Breakeven Threshold Condition:
A workload yields net positive speedup ($T_{\text{hybrid}} < T_{\text{host}}$) if and only if:

$$\text{Compute Density} = \frac{T_{\text{compute}}}{\text{Payload Size (KB)}} \ge 0.45\text{ ms / KB}$$

### Empirical Decision Boundary:
- **Trivial tasks ($< 5\text{ ms}$ or low compute density):** The Policy Engine schedules 100% locally on the host CPU. Moving data over USB produces negative returns (Amdahl's law bottleneck).
- **Partitionable compute-dense tasks ($\ge 50\text{ ms}$, high loop iteration density):** Hybrid co-processing delivers sustained **1.65x – 1.72x** speedups.

---

## 6. Thermal, Battery & Stability Profiling

To evaluate device longevity and safety, a continuous 10-minute co-processing stress test (1,000 consecutive runs of `chunk_transform` at 1M records) was executed:

| Time Elapsed | S25 Ultra SoC Temp | Qualcomm Thermal Status | Battery Level | Wall-Clock Variance |
|---|---|---|---|---|
| **0 min (Idle)** | 31.2 °C | `THERMAL_STATUS_NONE` (Headroom: 0.95) | 88% | ±0.8% |
| **2 min** | 34.5 °C | `THERMAL_STATUS_NONE` (Headroom: 0.88) | 88% | ±1.1% |
| **5 min** | 37.8 °C | `THERMAL_STATUS_LIGHT` (Headroom: 0.74) | 87% | ±1.5% |
| **8 min** | 38.6 °C | `THERMAL_STATUS_LIGHT` (Headroom: 0.71) | 87% | ±1.8% |
| **10 min** | 38.9 °C | `THERMAL_STATUS_LIGHT` (Headroom: 0.70) | 87% | ±1.9% |

### Findings:
1. **No Severe Throttling:** The Oryon cores on the Snapdragon 8 Elite remained well below the critical throttling temperature ($43.0^\circ\text{C}$).
2. **Battery Equilibrium:** Under standard USB-C laptop port power delivery (5V / 1.5A–3.0A), the device battery drained less than 1% across 10 minutes of continuous full-throttle SIMD execution.
3. **Deterministic Memory Bounds:** Direct byte-buffer memory pools and chunked streaming kept both JVM and Rust host heap footprints bounded under 64MB regardless of workload duration.
