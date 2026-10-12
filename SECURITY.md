# Security Policy

## Reporting Security Issues

PocketBridge is experimental software designed to run on paired, user-owned local devices. If you discover a vulnerability or security risk (such as arbitrary remote code execution, unauthenticated access, or directory traversal):

1. **Do not** report security vulnerabilities via public GitHub issues.
2. Please open a Private Vulnerability Advisory on GitHub or contact the maintainer directly.
3. Include details of the vulnerability, reproduction steps, and potential impact.

---

## Security Principles & Scope

- **Pairing & Local Scope:** PocketBridge daemons bind to `127.0.0.1` and ADB tunnels by default. No open network listeners should be exposed without explicit user consent.
- **No Arbitrary Execution:** Jobs must use strictly defined adapters and bounded parameters. Arbitrary shell commands are explicitly forbidden by the protocol.
- **Resource Limits:** The worker enforces hard limits on memory, execution time, and input/output payloads regardless of host instructions.
