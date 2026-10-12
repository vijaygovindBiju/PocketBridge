# PocketBridge — Experiments & Benchmarks

This document records the empirical benchmarking methodologies, feasibility experiments, and results across the USB-C link between the Linux host and the Android worker.

---

## 1. Objectives

1. Determine the baseline latency (RTT) of control messages over ADB-forwarded TCP (`127.0.0.1:9876`).
2. Measure round-trip data throughput across various payload sizes (64B, 1KB, 1MB, 10MB, 100MB).
3. Evaluate whether data transfer overhead can be amortized by task compute time.
4. Profile the thermal and memory impact on the Android device during sustained transfers.

---

## 2. Methodology & Instrumentation

- **Transport:** ADB TCP port forwarding over physical USB-C 3.x / USB 2.0 cables.
- **Framing:** Length-prefixed binary/JSON streams with constant memory $O(1)$ chunked streaming (64KB buffer).
- **Metrics Collected:**
  - `median_ms`, `p95_ms`, `p99_ms`, `min_ms`, `max_ms`, `stddev_ms`
  - Effective throughput in Mbps: `(2 * payload_size_bytes * 8) / (median_duration_sec * 1,000,000)`
- **Execution:** Automated via `pocketbridge bench --runs <N>` and recorded to timestamped JSON in `benchmarks/results/`.

---

## 3. Benchmark Categories

| Test Type | Payload Size | Primary Target Metric | Purpose |
|---|---|---|---|
| Latency Ping | 64 B | Round-trip time (ms) | Minimal control message latency |
| Small Ping | 1 KB | Round-trip time (ms) | Small job submission & status queries |
| Medium Transfer | 1 MB | Throughput (Mbps) | Small batch compute inputs |
| Large Transfer | 10 MB | Throughput (Mbps) | Medium data processing tasks |
| Bulk Transfer | 100 MB | Throughput & Memory stability | Heavy data offloading threshold |

---

## 4. Observations & Findings (Phase 0)

1. **Protocol Framing Efficiency:**
   Length-prefixed JSON with streaming raw payload prevents JVM and Host RAM ballooning. Testing up to 100MB round-trips operates in bounded memory buffers on both x86-64 Linux and Android ARM64.
2. **Thermal & Battery Monitoring:**
   During sustained transfers, monitoring Android `BatteryManager` and `PowerManager.currentThermalStatus` prevents aggressive thermal throttling before offloading compute.

---

## 5. Next Steps

- Measure comparative latency of ADB over Wi-Fi vs. USB-C.
- Run multi-run statistical sweeps on the target Samsung Galaxy S25 Ultra hardware.
- Test compute task offload amortization thresholds for Phase 1 vertical slices.
