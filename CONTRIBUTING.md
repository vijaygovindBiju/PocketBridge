# Contributing to PocketBridge

Thank you for your interest in contributing to PocketBridge!

## Guidelines

1. **Measurement First:** Any claims of compute acceleration or performance gains must include reproducible benchmark logs under `benchmarks/results/`.
2. **Fail-Safe Design:** All network and worker code must handle sudden disconnections gracefully without hanging or corrupting state.
3. **Rust Code Quality:**
   - Format with `cargo fmt`.
   - Ensure all warnings are addressed (`cargo clippy` or `cargo check`).
   - Run tests: `cargo test --workspace`.
4. **Android Worker:**
   - Maintain $O(1)$ memory streaming for data transfers.
   - Respect Android foreground service and thermal lifecycle restrictions.

## Submitting Pull Requests

1. Fork the repository and create a descriptive feature branch.
2. Ensure automated tests pass locally.
3. Submit a pull request with a clear description of changes and test evidence.
