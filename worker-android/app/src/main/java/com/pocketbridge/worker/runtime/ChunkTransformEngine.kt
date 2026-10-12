package com.pocketbridge.worker.runtime

import java.security.MessageDigest
import kotlinx.coroutines.ensureActive
import kotlin.coroutines.coroutineContext

object ChunkTransformEngine {
    const val ADAPTER_ID = "chunk_transform.v1"
    private const val MULTIPLIER_1 = -0x61C8864680B583EBL // 0x9E3779B97F4A7C15L in two's complement
    private const val MULTIPLIER_2 = 0x2545F4914F6CDD1DL

    fun transformRecord(index: Long, seed: Long, rounds: Int): Long {
        var state = seed + (index * MULTIPLIER_1)
        for (r in 0 until rounds) {
            state = state xor (state ushr 12)
            state = state xor (state shl 25)
            state = state xor (state ushr 27)
            state = state * MULTIPLIER_2
            state = java.lang.Long.rotateLeft(state, 13)
        }
        return state
    }

    suspend fun execute(
        recordCount: Long,
        rounds: Int,
        seed: Long,
        onProgress: suspend (Float) -> Unit
    ): Pair<String, Long> {
        val tStart = System.currentTimeMillis()
        val md = MessageDigest.getInstance("SHA-256")
        val bytes = ByteArray(8)

        val reportStep = (recordCount / 10).coerceAtLeast(1)

        for (i in 0 until recordCount) {
            if (i % 5000L == 0L) {
                coroutineContext.ensureActive() // Cooperative cancellation
            }
            if (i % reportStep == 0L) {
                onProgress(i.toFloat() / recordCount)
            }

            val v = transformRecord(i, seed, rounds)
            bytes[0] = (v ushr 56).toByte()
            bytes[1] = (v ushr 48).toByte()
            bytes[2] = (v ushr 40).toByte()
            bytes[3] = (v ushr 32).toByte()
            bytes[4] = (v ushr 24).toByte()
            bytes[5] = (v ushr 16).toByte()
            bytes[6] = (v ushr 8).toByte()
            bytes[7] = v.toByte()
            md.update(bytes)
        }

        val digest = md.digest()
        val sha256Hex = digest.joinToString("") { "%02x".format(it) }
        val durationMs = System.currentTimeMillis() - tStart
        return Pair(sha256Hex, durationMs)
    }
}
