package com.pocketbridge.worker.runtime

import java.security.MessageDigest
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlin.math.max

object DataAnalyticsEngine {
    const val ADAPTER_ID = "data_analytics.v1"
    const val MULTIPLIER_A = 0x517CC1B727220A95L
    const val MULTIPLIER_B = -0x61C8864680B583EBL // 0x9E3779B97F4A7C15L

    data class AnalyticsResult(
        val sha256Hex: String,
        val durationMs: Long,
        val totalPassed: Long,
        val xorAccum: Long
    )

    suspend fun execute(
        recordCount: Long,
        seed: Long,
        filterMin: Long,
        filterMax: Long,
        onProgress: suspend (Float) -> Unit
    ): AnalyticsResult = coroutineScope {
        val tStart = System.currentTimeMillis()
        val md = MessageDigest.getInstance("SHA-256")

        val chunkSize = 32768L
        val totalChunks = ((recordCount + chunkSize - 1) / chunkSize).toInt()

        val deferredChunks = (0 until totalChunks).map { chunkIdx ->
            val start = chunkIdx * chunkSize
            val end = minOf(start + chunkSize, recordCount)

            async(Dispatchers.Default) {
                val byteList = ArrayList<Byte>(((end - start) * 4).toInt())
                var passed = 0L
                var xorAcc = 0L

                for (i in start until end) {
                    val vRaw = (seed + (i * MULTIPLIER_A)) xor ((i * MULTIPLIER_B) ushr 17)
                    val v = vRaw
                    if (v in filterMin..filterMax) {
                        byteList.add((v ushr 56).toByte())
                        byteList.add((v ushr 48).toByte())
                        byteList.add((v ushr 40).toByte())
                        byteList.add((v ushr 32).toByte())
                        byteList.add((v ushr 24).toByte())
                        byteList.add((v ushr 16).toByte())
                        byteList.add((v ushr 8).toByte())
                        byteList.add(v.toByte())
                        passed++
                        xorAcc = xorAcc xor vRaw
                    }
                }
                val byteArray = ByteArray(byteList.size)
                for (bIdx in 0 until byteList.size) {
                    byteArray[bIdx] = byteList[bIdx]
                }
                Triple(byteArray, passed, xorAcc)
            }
        }

        var totalPassed = 0L
        var totalXor = 0L
        val reportStep = max(1, totalChunks / 10)
        var done = 0

        for (deferred in deferredChunks) {
            val (chunkBytes, passed, xorAcc) = deferred.await()
            md.update(chunkBytes)
            totalPassed += passed
            totalXor = totalXor xor xorAcc
            done++

            if (done % reportStep == 0 || done == totalChunks) {
                onProgress(done.toFloat() / totalChunks)
            }
        }

        val digest = md.digest()
        val sha256Hex = digest.joinToString("") { "%02x".format(it) }
        val durationMs = System.currentTimeMillis() - tStart

        AnalyticsResult(sha256Hex, durationMs, totalPassed, totalXor)
    }
}
