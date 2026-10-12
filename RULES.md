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
