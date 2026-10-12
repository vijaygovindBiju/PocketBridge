# Security and Privacy

## 1. Threat model

PocketBridge connects a user-owned Linux computer to a phone and transfers workload inputs and outputs. Risks include an unauthorized client submitting jobs, a compromised host sending unsafe instructions, malicious or malformed inputs, path traversal, unbounded resource consumption, accidental disclosure of files, stale pairing credentials, and a device disconnect leaving partial artifacts behind.

The initial threat model is a single user pairing their own devices. Do not expose the worker to a public or untrusted network in the initial releases.

## 2. Security principles

- Pairing requires an explicit user action on the device and host.
- The worker accepts jobs only from an authenticated paired host.
- ADB debugging authorization is not a substitute for PocketBridge's own application-level authentication and job policy.
- Bind listeners to loopback or the ADB tunnel unless a later transport design explicitly requires otherwise.
- Do not run the worker as root.
- Do not accept shell command strings as a generic job format.
- Use named adapters and structured parameters, not arbitrary command-line input.
- Enforce job limits on the worker, even if the host validates them too.
- Keep network access disabled for remote jobs by default.
- Validate every path and artifact name; use per-job private temporary directories.
- Clean partial input/output on cancellation or expiration.
- Store the minimum telemetry and content necessary for debugging.
- Provide pause, disconnect, revoke, and data cleanup controls.

## 3. Sandboxing expectations

Android and Termux are not equivalent to a strong hostile-code sandbox. For MVP, accept only bundled or explicitly installed adapters, with allowlisted operations and strict resource limits. Do not permit arbitrary user-submitted executable code from remote sources. Before adding script execution, evaluate isolation options, Android process boundaries, storage permissions, and attack surface.

## 4. Authentication and pairing

Initial pairing should establish a fresh random secret or public-key identity using a short, user-confirmed pairing flow. Store credentials using platform-appropriate protected storage. Support revocation from both sides. Every session authenticates before job data is exchanged. Never print long-lived secrets in logs.

A later version should use a reviewed authenticated transport or message authentication scheme, with replay protection and explicit key rotation. Do not invent cryptography; use established libraries and obtain review before treating it as production-secure.

## 5. Data handling

- Show the adapter, paths/artifact categories, sizes, and purpose before sensitive data is sent.
- Do not scan or upload a user's whole home directory.
- Never transfer another application's process memory by default.
- Apply retention limits for artifacts and logs.
- Permit the user to clear all task artifacts and pairing information.
- Treat results from the worker as untrusted until size/hash/schema and adapter-specific validation complete.

## 6. Resource-abuse controls

Every job must have maximum input size, output size, memory budget, CPU/process limit where enforceable, and wall-clock deadline. Limit concurrent jobs. Reject requests beyond the worker's advertised policy. Respect thermal and battery safety signals; stop or throttle gracefully rather than keeping the phone at unsafe sustained load.

## 7. Release-blocking risks

- Any unauthenticated route to execution.
- Arbitrary shell command execution from the protocol.
- Path traversal outside the task workspace.
- Secret leakage in logs or crash reports.
- Unbounded input/output or memory consumption.
- Silent job duplication for non-idempotent tasks.
- A remote listener accidentally exposed to the broader network.
