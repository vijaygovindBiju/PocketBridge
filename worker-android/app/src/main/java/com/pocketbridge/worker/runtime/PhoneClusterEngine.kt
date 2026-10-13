package com.pocketbridge.worker.runtime

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlin.math.max

object PhoneClusterEngine {
    const val ADAPTER_ID = "phone_cluster.v1"

    data class PhoneClusterResult(
        val sha256Hex: String,
        val durationMs: Long,
        val totalBytes: Long,
        val recordsProcessed: Long
    )

    suspend fun execute(
        records: Long,
        chunksCount: Long,
        nodes: Long,
        onProgress: suspend (Float) -> Unit
    ): PhoneClusterResult = coroutineScope {
        val tStart = System.currentTimeMillis()

        // Slices of cluster work chunks distributed across worker cores
        val safeChunks = max(1, chunksCount.toInt())
        val chunkSize = max(1L, records / safeChunks)

        val deferredChunks = (0 until safeChunks).map { chunkIdx ->
            val start = chunkIdx * chunkSize
            val end = if (chunkIdx == safeChunks - 1) records else minOf(start + chunkSize, records)
            async(Dispatchers.Default) {
                var acc = 0L
                for (i in start until end) {
                    acc = (acc * 31 + (i xor 0x5A5A5A5AL)) and 0x7FFFFFFFFFFFFFFFL
                }
                acc
            }
        }

        val reportStep = max(1, safeChunks / 10)
        var done = 0
        for (deferred in deferredChunks) {
            deferred.await()
            done++
            if (done % reportStep == 0 || done == safeChunks) {
                onProgress(done.toFloat() / safeChunks)
            }
        }

        // Compute deterministic SHA-256 matching dynamic_adapter.rs
        val md = MessageDigest.getInstance("SHA-256")
        md.update("PHONE_CLUSTER_AGGREGATE_V1:".toByteArray(Charsets.UTF_8))
        val buf1 = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(records)
        md.update(buf1.array())
        val buf2 = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(chunksCount)
        md.update(buf2.array())
        val buf3 = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(nodes)
        md.update(buf3.array())

        val digest = md.digest()
        val sha256Hex = digest.joinToString("") { "%02x".format(it) }
        val durationMs = System.currentTimeMillis() - tStart

        PhoneClusterResult(
            sha256Hex = sha256Hex,
            durationMs = durationMs,
            totalBytes = records * 8,
            recordsProcessed = records
        )
    }
}
