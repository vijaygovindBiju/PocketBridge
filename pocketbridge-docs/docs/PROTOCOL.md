# Protocol Draft

**Status:** Initial proposal; versioning and serialization are not implemented yet.

## 1. Goals

The protocol must be small, explicit, versioned, authenticated, stream large inputs safely, and support cancellation and reconnects. It must not depend on a permanent TCP connection being available.

Recommended starting point: length-prefixed messages over a local authenticated stream, with a compact versioned envelope. JSON is acceptable for the first prototype because it is easy to inspect; benchmark binary payload framing separately rather than embedding large binary data as base64 in JSON. A binary serialization format can be considered once the schema stabilizes.

## 2. Envelope

Every control message should include:

```json
{
  "protocol_version": 1,
  "message_id": "unique-message-id",
  "session_id": "paired-session-id",
  "kind": "message-kind",
  "sent_at_unix_ms": 0,
  "payload": {}
}
```

This is a schema illustration, not working code. Production code must validate maximum lengths, allowed kinds, missing fields, and version compatibility.

## 3. Initial message kinds

- `HELLO` — protocol and agent version negotiation.
- `AUTH_CHALLENGE` / `AUTH_PROOF` — establish an authenticated session.
- `CAPABILITIES` — report ABI, runtimes, resource budgets, worker policy, and supported adapters.
- `HEARTBEAT` — report liveness and optional resource summary.
- `SUBMIT_JOB` — request job creation with an adapter ID and payload manifest.
- `JOB_ACCEPTED` / `JOB_REJECTED` — confirm acceptance or give a structured reason.
- `INPUT_CHUNK` / `INPUT_COMPLETE` — stream a bounded input artifact with offset/hash.
- `JOB_STARTED` / `JOB_PROGRESS` — report execution status.
- `CANCEL_JOB` / `CANCELLED` — request and confirm cancellation.
- `RESULT_MANIFEST` / `RESULT_CHUNK` / `RESULT_COMPLETE` — return output artifacts with hashes.
- `JOB_ERROR` — report a typed failure.
- `GOODBYE` — close a session cleanly.

## 4. Job contract

Example conceptual job request:

```json
{
  "job_id": "uuid",
  "adapter_id": "example.chunk-transform.v1",
  "input_manifest": [
    {"artifact_id":"input-0","size_bytes":1024,"sha256":"..."}
  ],
  "requirements": {
    "architecture": "aarch64",
    "runtime": "adapter-defined",
    "min_memory_bytes": 67108864,
    "max_wall_time_ms": 30000
  },
  "limits": {
    "max_input_bytes": 10485760,
    "max_output_bytes": 10485760,
    "max_processes": 1,
    "network_access": false
  },
  "deadline_unix_ms": 0,
  "idempotent": true,
  "data_classification": "user-approved"
}
```

All values are illustrative. Use real unique IDs, validate all resource limits on the worker, and do not trust host-side estimates alone. A worker must enforce its own maximum limits because the host can be compromised or buggy.

## 5. Result contract

A result includes:
- job ID and attempt ID;
- final state (`succeeded`, `failed`, `cancelled`, `timed_out`);
- output artifact manifest, size, and cryptographic hash;
- worker-reported start/end times and metrics;
- error code and non-sensitive diagnostic summary when applicable;
- adapter-level result metadata required for validation.

The host should validate hashes, size quotas, schema, and the adapter-specific output invariant before declaring success.

## 6. Idempotency and retries

A network acknowledgement can be lost even when a worker received a job. Use job IDs and attempt IDs, plus a worker-side job table, to prevent accidental duplicate execution. Only retry automatically when the job is idempotent or the adapter provides a deduplication mechanism. Do not assume at-least-once message delivery means exactly-once execution.

## 7. Flow control

- Bound the number of concurrent jobs and transfers.
- Bound chunk size and total artifact size.
- Apply backpressure if the worker is receiving faster than it can write or execute.
- Prefer streaming artifacts over loading entire payloads into memory.
- Check hashes for transferred artifacts.
- Support cancellation of transfers and remove partial artifacts after a defined expiry.

## 8. Versioning

- Protocol major version mismatch: refuse session.
- Unknown optional capability: ignore only if declared optional.
- Unknown message kind: return a structured unsupported-message error and close if safe parsing cannot continue.
- Worker advertises an explicit set of adapters and adapter versions; the host must not assume compatibility from a generic version string.
