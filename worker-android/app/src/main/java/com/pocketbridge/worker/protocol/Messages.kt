package com.pocketbridge.worker.protocol

import com.google.gson.JsonElement
import com.google.gson.annotations.SerializedName

data class HelloPayload(
    @SerializedName("agent_name")
    val agentName: String = "pocketbridge-android",

    @SerializedName("agent_version")
    val agentVersion: String = "0.1.0",

    @SerializedName("protocol_version_min")
    val protocolVersionMin: Int = 1,

    @SerializedName("protocol_version_max")
    val protocolVersionMax: Int = 1,

    @SerializedName("role")
    val role: String = "worker"
)

data class HeartbeatPayload(
    @SerializedName("uptime_ms")
    val uptimeMs: Long,

    @SerializedName("cpu_usage_percent")
    val cpuUsagePercent: Double? = null,

    @SerializedName("memory_used_bytes")
    val memoryUsedBytes: Long? = null,

    @SerializedName("memory_total_bytes")
    val memoryTotalBytes: Long? = null,

    @SerializedName("temperature_celsius")
    val temperatureCelsius: Double? = null,

    @SerializedName("battery_percent")
    val batteryPercent: Int? = null,

    @SerializedName("battery_charging")
    val batteryCharging: Boolean? = null,

    @SerializedName("thermal_status")
    val thermalStatus: String? = null
)

data class BenchmarkEchoPayload(
    @SerializedName("sequence")
    val sequence: Long,

    @SerializedName("binary_size_bytes")
    val binarySizeBytes: Long,

    @SerializedName("sent_at_unix_ms")
    val sentAtUnixMs: Long
)

data class BenchmarkEchoReplyPayload(
    @SerializedName("sequence")
    val sequence: Long,

    @SerializedName("binary_size_bytes")
    val binarySizeBytes: Long,

    @SerializedName("original_sent_at_unix_ms")
    val originalSentAtUnixMs: Long,

    @SerializedName("worker_received_at_unix_ms")
    val workerReceivedAtUnixMs: Long,

    @SerializedName("worker_replied_at_unix_ms")
    val workerRepliedAtUnixMs: Long
)

// Job Lifecycle Data Classes

data class SubmitJobPayload(
    @SerializedName("job_id") val jobId: String,
    @SerializedName("adapter_id") val adapterId: String,
    @SerializedName("input_parameters") val inputParameters: JsonElement,
    @SerializedName("deadline_unix_ms") val deadlineUnixMs: Long = 0,
    @SerializedName("idempotent") val idempotent: Boolean = true,
    @SerializedName("priority") val priority: Int = 1
)

data class ChunkTransformParams(
    @SerializedName("record_count") val recordCount: Long = 50000,
    @SerializedName("rounds") val rounds: Int = 20,
    @SerializedName("seed") val seed: Long = 0x123456789ABCDEF0L
)

data class JobAcceptedPayload(
    @SerializedName("job_id") val jobId: String,
    @SerializedName("attempt_id") val attemptId: String,
    @SerializedName("accepted_at_unix_ms") val acceptedAtUnixMs: Long = System.currentTimeMillis()
)

data class JobRejectedPayload(
    @SerializedName("job_id") val jobId: String,
    @SerializedName("reason") val reason: String,
    @SerializedName("code") val code: String
)

data class JobStartedPayload(
    @SerializedName("job_id") val jobId: String,
    @SerializedName("started_at_unix_ms") val startedAtUnixMs: Long = System.currentTimeMillis()
)

data class JobProgressPayload(
    @SerializedName("job_id") val jobId: String,
    @SerializedName("progress_fraction") val progressFraction: Float,
    @SerializedName("memory_used_bytes") val memoryUsedBytes: Long? = null,
    @SerializedName("elapsed_compute_ms") val elapsedComputeMs: Long = 0
)

data class CancelJobPayload(
    @SerializedName("job_id") val jobId: String,
    @SerializedName("reason") val reason: String
)

data class JobCancelledPayload(
    @SerializedName("job_id") val jobId: String,
    @SerializedName("cancelled_at_unix_ms") val cancelledAtUnixMs: Long = System.currentTimeMillis()
)

data class ResultManifestPayload(
    @SerializedName("job_id") val jobId: String,
    @SerializedName("adapter_id") val adapterId: String,
    @SerializedName("status") val status: String = "succeeded",
    @SerializedName("output_size_bytes") val outputSizeBytes: Long,
    @SerializedName("output_sha256") val outputSha256: String,
    @SerializedName("compute_duration_ms") val computeDurationMs: Long,
    @SerializedName("records_processed") val recordsProcessed: Long,
    @SerializedName("metadata") val metadata: JsonElement? = null
)

data class JobErrorPayload(
    @SerializedName("job_id") val jobId: String,
    @SerializedName("error_code") val errorCode: String,
    @SerializedName("message") val message: String
)
