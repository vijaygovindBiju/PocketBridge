# PocketBridge — Project Rules & Engineering Guidelines

This document outlines the core architectural rules, engineering standards, and contribution guidelines that govern all development on PocketBridge.

---

## 🏛️ Core Architectural Rules

1. **Explicit Support Beats Magic:**
   - Do not claim or attempt to provide "transparent hardware pooling" (e.g. merging phone RAM into the Linux host or treating the phone GPU as a native Linux desktop GPU).
   - A workload can only be offloaded if it has a defined **Workload Adapter** with clear input/output contracts, serialization rules, and execution constraints.

2. **Measure Before Claiming Speedup:**
   - Every speedup calculation must include end-to-end overhead:
     $$\text{Speedup} = \frac{\text{Local Duration}}{\text{Remote Total Duration}}$$
   - $\text{Remote Total Duration}$ **must** include input transfer time, remote startup/queue time, remote compute time, and result transfer time. Compute-only times must not be cherry-picked.

3. **Fail-Safe by Default:**
   - Unexpected disconnections (e.g. cable unplugged, worker process terminated) must never leave the host daemon hanging or mark an unverified job as successful.
   - Non-idempotent jobs must never be blindly retried without explicit deduplication keys.

4. **Privacy & Security by Default:**
   - Listeners must bind strictly to `127.0.0.1` (loopback) and ADB forwarded tunnels. Never expose public network listeners by default.
   - Arbitrary shell command strings are strictly forbidden over the wire protocol.
   - Worker processes must enforce resource bounds (memory, CPU, timeout) independently of host claims.

5. **Bounded Resource Usage ($O(1)$ Memory Streaming):**
   - Both host and worker must stream large data payloads using chunked buffers (e.g. 64 KB). Never buffer 50MB–100MB+ payloads entirely into a single JSON string or JVM array to prevent OutOfMemory crashes.

---

## 🛠️ Code & Implementation Standards

### Rust (Host Workspace)
- All shared data models, protocol framing, and adapters belong in `pocketbridge-core`.
- Application presentation (TUI, CLI) belongs in `pocketbridge-host`.
- Format code before committing: `cargo fmt`.
- Zero compiler warnings allowed: `cargo check --workspace` must pass cleanly.
- All new features must include unit or integration tests: `cargo test --workspace`.

### Kotlin (Android Worker)
- Use Kotlin coroutines with cooperative cancellation (`coroutineContext.ensureActive()`).
- Background tasks must be anchored to the Android `WorkerService` foreground service lifecycle with user-visible persistent notifications.
- Device profiling must use official platform APIs (`ActivityManager`, `BatteryManager`, `PowerManager`).

---

## 📁 Repository & File Structure Rules

PocketBridge strictly adheres to a modular, separated architecture. All contributions must place files in their designated domains according to the following layout:

```text
PocketBridge/
├── README.md                 # 30-second overview, architecture overview, quickstart
├── RULES.md                  # Authoritative architectural rules, structure, and git hygiene
├── PHASES.md                 # Phase-by-phase implementation progress & roadmap tracker
├── LICENSE                   # Open-source MIT license
├── CONTRIBUTING.md            # Contribution workflow and guidelines
├── SECURITY.md               # Security policy, threat model, and vulnerability reporting
├── .gitignore                # Rust target, Android build, Gradle, and local SDK artifacts
│
├── host/                     # Rust Multi-Crate Workspace
│   ├── Cargo.toml            # Workspace manifest definition
│   ├── pocketbridge-core/    # PURE ENGINE: wire protocol, framing, profiler, adapters, tests
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── protocol/     # Envelope models, message framing (FramedSocket), validation
│   │   │   ├── profiler/     # Host Linux hardware metrics (/proc/meminfo, CPU, uname)
│   │   │   ├── adapter/      # Offloadable workload adapters (e.g. chunk_transform.rs)
│   │   │   ├── runner/       # Benchmark suite and latency/throughput measurement
│   │   │   └── client/       # Remote worker client and ADB tunnel connector
│   │   └── tests/            # Mock TCP integration and unit tests
│   │
│   └── pocketbridge-host/    # USER PRESENTATION: CLI binary and interactive Ratatui TUI
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs       # CLI entrypoint and subcommand dispatcher
│           ├── app.rs        # Interactive application state machine
│           └── ui.rs         # Ratatui terminal UI rendering and layout components
│
├── worker-android/           # Native Android Kotlin Application
│   ├── build.gradle.kts      # Root Android build configuration
│   ├── gradle.properties     # JVM network flags and SDK settings
│   └── app/
│       ├── build.gradle.kts  # Jetpack Compose and coroutine dependencies
│       └── src/main/
│           ├── AndroidManifest.xml # Permissions (Foreground service, wake lock)
│           └── java/com/pocketbridge/worker/
│               ├── MainActivity.kt # Jetpack Compose UI container
│               ├── service/        # WorkerService foreground daemon running TCP server
│               ├── runtime/        # Kotlin compute engines (ChunkTransformEngine.kt)
│               ├── model/          # Wire protocol data classes matching Rust envelopes
│               ├── profiler/       # Android device metrics (ActivityManager, BatteryManager)
│               └── ui/             # Compose theme, screens, and viewmodels
│
├── docs/                     # In-Depth Engineering Documentation
│   ├── PROJECT.md            # Vision, scope boundaries, and non-goals
│   ├── ARCHITECTURE.md       # Component architecture, transport layer, failure modes
│   ├── PROTOCOL.md           # Wire protocol schemas, frame headers, and error codes
│   ├── ROADMAP.md            # Long-term multi-phase roadmap
│   ├── PHASES.md             # Completed vs. upcoming phase deliverables tracker
│   ├── DECISIONS.md          # Architecture Decision Records (ADRs)
│   └── EXPERIMENTS.md        # Benchmark methodology and empirical results
│
├── benchmarks/results/       # Empirical benchmark JSON outputs and performance logs
└── .github/workflows/        # CI/CD Workflows
    └── ci.yml                # Automated GitHub Actions (Rust tests + Android Gradle check)
```

### Directory Placement Constraints

1. **Separation of Core vs. Presentation:**
   - `pocketbridge-core` **must never** import terminal UI libraries (`ratatui`, `crossterm`). It must remain a reusable library capable of running headless or being embedded into background daemons.
   - All interactive terminal code and CLI argument parsing belongs strictly in `pocketbridge-host`.

2. **Workload Adapter Symmetry:**
   - Any new offloadable workload must implement matching symmetric logic in both:
     - Rust: `host/pocketbridge-core/src/adapter/<adapter_name>.rs`
     - Android: `worker-android/app/src/main/java/com/pocketbridge/worker/runtime/<AdapterName>Engine.kt`
   - Both implementations must pass deterministic cross-platform bit-equivalence tests.

3. **Documentation Integrity:**
   - If protocol framing or message types change, update `docs/PROTOCOL.md` and `docs/ARCHITECTURE.md` in the exact same commit.
   - Milestone progress must be updated in `PHASES.md`.

---

## 🌿 Git & Commit Standards

1. **Commit at Necessary Checkpoints:**
   - Commit code when a feature, bug fix, or phase milestone is verified and working.
   - Do not bundle unrelated architectural changes into a single mega-commit.

2. **Conventional Commit Messages:**
   - `feat(...)`: New feature or capability
   - `fix(...)`: Bug fix or error resolution
   - `docs(...)`: Documentation updates
   - `test(...)`: Adding or updating test suites
   - `chore(...)`: Tooling, build scripts, or dependency updates

3. **Keep the Tree Clean:**
   - Never commit generated build artifacts (`host/target/`, `worker-android/**/build/`, `.gradle/`, `local.properties`).

