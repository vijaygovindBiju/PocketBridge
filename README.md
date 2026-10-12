# PocketBridge

Opportunistic compute offloading from a Linux laptop to an attached Android phone over USB-C.

---

## Overview

PocketBridge explores whether an attached Android phone can contribute spare compute capacity to an Arch Linux laptop. Rather than making impossible promises about transparent physical RAM pooling, PocketBridge moves bounded, explicitly supported jobs across a high-speed, local link (ADB over USB-C) and benchmarks whether offloading was actually faster.

- **Host (Laptop):** Rust daemon with an interactive TUI (`ratatui`) & CLI to schedule jobs, profile hardware, and benchmark the link.
- **Worker (Phone):** Native Kotlin Android app with a foreground service running a streaming TCP daemon on `127.0.0.1:9876`.
- **Link:** ADB-forwarded TCP over USB-C — localhost-only, fast, and secure.

---

## Project Structure

```text
PocketBridge/
├── host/                    # Rust Cargo workspace
│   ├── pocketbridge-core/   # Protocol, framing, profiler, and benchmark runner
│   └── pocketbridge-host/   # Ratatui TUI dashboard & CLI binary
├── worker-android/          # Android Kotlin app (Compose UI + Foreground Service)
├── benchmarks/results/      # Benchmark JSON reports
└── pocketbridge-docs/       # Architecture, Protocol, and Design decisions
```

---

## Quickstart

### 1. Build and Run the Host (Linux)

```bash
# Run the interactive TUI
cd host
cargo run --release

# Or use CLI commands
cargo run --release -- profile   # Inspect host & phone hardware
cargo run --release -- connect   # Handshake with worker
cargo run --release -- bench     # Run link latency & throughput benchmarks
```

### 2. Build the Android Worker

```bash
cd worker-android
./gradlew assembleDebug

# Install on connected phone
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

---

## Documentation

Full specifications and architecture design docs are in [`pocketbridge-docs/docs/`](pocketbridge-docs/docs/):
- [Architecture](pocketbridge-docs/docs/ARCHITECTURE.md)
- [Protocol Specification](pocketbridge-docs/docs/PROTOCOL.md)
- [Security & Threat Model](pocketbridge-docs/docs/SECURITY.md)
- [Validation & Test Plan](pocketbridge-docs/docs/TEST_PLAN.md)
- [Roadmap](pocketbridge-docs/docs/ROADMAP.md)

---

## License

MIT
