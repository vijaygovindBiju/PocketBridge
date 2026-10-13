# Wire Protocol Specification (v1)

## 1. Framing Format

All messages transmitted across the ADB forwarded stream (`127.0.0.1:9876`) use an 8-byte binary framing header followed by a variable-length JSON payload:

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     Payload Length (u32 BE)                   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Magic Byte 0x50 ('P') |          Magic Byte 0x4B ('K') |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Magic Byte 0x54 ('T') |          Magic Byte 0x42 ('B') |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                 JSON Envelope Payload (Bytes)                 |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **Payload Length:** 32-bit big-endian integer specifying exact length of JSON envelope in bytes (max 64 MB).
- **Magic Bytes:** `0x50 0x4B 0x54 0x42` (ASCII `"PKTB"`). Connections with invalid magic headers are immediately rejected.

---

## 2. Envelope Schema

```json
{
  "version": 1,
  "message_id": "c1f7b02e-4b68-45d2-a7f4-8848d1e2e92c",
  "session_id": "s-8f3e1a0b",
  "kind": "SUBMIT_JOB",
  "sent_at_unix_ms": 1728834920150,
  "payload": { ... }
}
```

---

## 3. Implemented Message Kinds

### Connection Lifecycle & Discovery
- `HELLO` — Initiates connection; negotiates protocol version and agent role (`host` or `worker`).
- `HEARTBEAT` — Transmitted periodically; reports battery level, charging state, thermal headroom status, and active job count.
- `GOODBYE` — Graceful connection termination.

### Job Orchestration
- `SUBMIT_JOB` — Requests execution of a workload adapter (`adapter_id`, input JSON parameters, limits).
- `JOB_ACCEPTED` — Worker acknowledges job acceptance with an assigned attempt ID.
- `JOB_REJECTED` — Worker rejects job (unsupported adapter, insufficient RAM, thermal throttle).
- `JOB_STARTED` — Worker confirms execution commencement.
- `JOB_PROGRESS` — Emitted during long-running tasks reporting fractional progress (`0.0` to `1.0`) and elapsed compute duration.
- `RESULT_MANIFEST` — Final execution result containing output size, compute duration, records processed, and SHA-256 digest.
- `CANCEL_JOB` — Host requests termination of an active job.
- `CANCELLED` — Worker confirms cancellation and releases resources/WakeLocks.
- `JOB_ERROR` — Reports execution or parsing failure.

### Link Benchmarking
- `BENCHMARK_ECHO` — Host sends arbitrary test payload bytes to measure round-trip latency.
- `BENCHMARK_ECHO_REPLY` — Worker returns echoed bytes immediately with remote timestamp.

---

## 4. Workload Adapter Payloads

### Core Built-in: `chunk_transform.v1`
```json
{
  "record_count": 50000,
  "rounds": 20,
  "seed": 1311768467294899696
}
```
*Output:* 64-bit record transformations verified via SHA-256 hash.

### Plugins: `image_batch.v1`
```json
{
  "image_count": 100,
  "width": 256,
  "height": 256,
  "seed": 1311768467294899696,
  "operation": "grayscale_invert"
}
```

### Plugins: `data_analytics.v1`
```json
{
  "record_count": 50000,
  "seed": 1311768467294899696,
  "filter_min": 0,
  "filter_max": 9223372036854775807
}
```
