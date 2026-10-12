package com.pocketbridge.worker.protocol

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
