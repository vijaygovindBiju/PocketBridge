package com.pocketbridge.worker.runtime

import java.security.MessageDigest
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.ensureActive
import kotlin.coroutines.coroutineContext

object ChunkTransformEngine {
    const val ADAPTER_ID = "chunk_transform.v1"
    const val MULTIPLIER_1 = -0x61C8864680B583EBL // 0x9E3779B97F4A7C15L in two's complement
    const val MULTIPLIER_2 = 0x2545F4914F6CDD1DL

    private var nativeLoaded = false

    init {
        try {
            System.loadLibrary("pocketbridge_native")
            nativeLoaded = true
            android.util.Log.i("ChunkTransformEngine", "Loaded native C++ acceleration engine (pocketbridge_native.so)")
        } catch (t: Throwable) {
            android.util.Log.w("ChunkTransformEngine", "Native library not available, falling back to Kotlin: ${t.message}")
            nativeLoaded = false
        }
    }

    @JvmStatic
    private external fun nativeTransform(recordCount: Long, rounds: Int, seed: Long): String

    @JvmStatic
    inline fun transformRecord(index: Long, seed: Long, rounds: Int): Long {
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
    ): Pair<String, Long> = coroutineScope {
        val tStart = System.currentTimeMillis()

        if (nativeLoaded) {
            onProgress(0.1f)
            val shaHex = kotlinx.coroutines.withContext(Dispatchers.Default) {
                nativeTransform(recordCount, rounds, seed)
            }
            onProgress(1.0f)
            val durationMs = System.currentTimeMillis() - tStart
            return@coroutineScope Pair(shaHex, durationMs)
        }

        val md = MessageDigest.getInstance("SHA-256")

        // Chunk into parallel batches to distribute across all Snapdragon 8 Elite CPU cores
        val chunkSize = 32768L // 32K records per chunk = 256 KB per slice
        val totalChunks = ((recordCount + chunkSize - 1) / chunkSize).toInt()

        // Launch parallel chunk computations on Dispatchers.Default (uses all 8 cores)
        val deferredChunks = (0 until totalChunks).map { chunkIdx ->
            val start = chunkIdx * chunkSize
            val count = minOf(chunkSize, recordCount - start).toInt()

            async(Dispatchers.Default) {
                val chunkBytes = ByteArray(count * 8)
                var offset = 0
                for (i in 0 until count) {
                    val r = start + i
                    val v = transformRecord(r, seed, rounds)
                    chunkBytes[offset] = (v ushr 56).toByte()
                    chunkBytes[offset + 1] = (v ushr 48).toByte()
                    chunkBytes[offset + 2] = (v ushr 40).toByte()
                    chunkBytes[offset + 3] = (v ushr 32).toByte()
                    chunkBytes[offset + 4] = (v ushr 24).toByte()
                    chunkBytes[offset + 5] = (v ushr 16).toByte()
                    chunkBytes[offset + 6] = (v ushr 8).toByte()
                    chunkBytes[offset + 7] = v.toByte()
                    offset += 8
                }
                chunkBytes
            }
        }

        // Ingest chunks in strict sequential order into the SHA-256 hasher
        var recordsIngested = 0L
        val reportStep = (recordCount / 10).coerceAtLeast(1)
        var lastReported = 0L

        for (deferred in deferredChunks) {
            coroutineContext.ensureActive()
            val chunkBytes = deferred.await()
            md.update(chunkBytes)
            recordsIngested += chunkBytes.size / 8

            if (recordsIngested - lastReported >= reportStep || recordsIngested == recordCount) {
                lastReported = recordsIngested
                onProgress(recordsIngested.toFloat() / recordCount)
            }
        }

        val digest = md.digest()
        val sha256Hex = digest.joinToString("") { "%02x".format(it) }
        val durationMs = System.currentTimeMillis() - tStart
        Pair(sha256Hex, durationMs)
    }
}
