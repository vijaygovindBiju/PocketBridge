package com.pocketbridge.worker.protocol

import com.google.gson.JsonElement
import com.google.gson.annotations.SerializedName
import java.util.UUID

data class Envelope(
    @SerializedName("protocol_version")
    val protocolVersion: Int = 1,

    @SerializedName("message_id")
    val messageId: String = UUID.randomUUID().toString(),

    @SerializedName("session_id")
    val sessionId: String = "",

    @SerializedName("kind")
    val kind: String,

    @SerializedName("sent_at_unix_ms")
    val sentAtUnixMs: Long = System.currentTimeMillis(),

    @SerializedName("priority")
    val priority: Int? = null,

    @SerializedName("payload")
    val payload: JsonElement? = null
)

object MessageKind {
    const val HELLO = "HELLO"
    const val HEARTBEAT = "HEARTBEAT"
    const val BENCHMARK_ECHO = "BENCHMARK_ECHO"
    const val BENCHMARK_ECHO_REPLY = "BENCHMARK_ECHO_REPLY"
    const val GOODBYE = "GOODBYE"

    // Job Lifecycle Kinds
    const val SUBMIT_JOB = "SUBMIT_JOB"
    const val JOB_ACCEPTED = "JOB_ACCEPTED"
    const val JOB_REJECTED = "JOB_REJECTED"
    const val JOB_STARTED = "JOB_STARTED"
    const val JOB_PROGRESS = "JOB_PROGRESS"
    const val CANCEL_JOB = "CANCEL_JOB"
    const val CANCELLED = "CANCELLED"
    const val RESULT_MANIFEST = "RESULT_MANIFEST"
    const val JOB_ERROR = "JOB_ERROR"
}
