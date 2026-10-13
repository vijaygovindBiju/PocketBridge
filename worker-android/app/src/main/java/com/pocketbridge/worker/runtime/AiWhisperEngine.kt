package com.pocketbridge.worker.runtime

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlin.math.max
import kotlin.math.roundToLong

object AiWhisperEngine {
    const val ADAPTER_ID = "ai_whisper.v1"

    data class WhisperResult(
        val sha256Hex: String,
        val durationMs: Long,
        val totalBytes: Long,
        val tokensGenerated: Long,
        val audioSeconds: Double
    )

    suspend fun execute(
        samplesCount: Long,
        model: String,
        language: String,
        onProgress: suspend (Float) -> Unit
    ): WhisperResult = coroutineScope {
        val tStart = System.currentTimeMillis()

        // 16-bit PCM audio samples chunking (simulate Mel filterbank + transformer decoder)
        val chunkSize = 32000L // 2 seconds chunks at 16kHz
        val totalChunks = max(1, ((samplesCount + chunkSize - 1) / chunkSize).toInt())

        val deferredChunks = (0 until totalChunks).map { chunkIdx ->
            val start = chunkIdx * chunkSize
            val end = minOf(start + chunkSize, samplesCount)
            async(Dispatchers.Default) {
                var sum = 0.0
                for (i in start until end) {
                    val sampleVal = ((i * 1103515245L + 12345L) and 0xFFFFL) - 32768
                    sum += sampleVal * sampleVal
                }
                sum
            }
        }

        val reportStep = max(1, totalChunks / 10)
        var done = 0
        for (deferred in deferredChunks) {
            deferred.await()
            done++
            if (done % reportStep == 0 || done == totalChunks) {
                onProgress(done.toFloat() / totalChunks)
            }
        }

        // Compute deterministic SHA-256 matching dynamic_adapter.rs
        val md = MessageDigest.getInstance("SHA-256")
        md.update("WHISPER_AUDIO_TRANSCRIPT_V1:".toByteArray(Charsets.UTF_8))
        val buf = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(samplesCount)
        md.update(buf.array())
        md.update(model.toByteArray(Charsets.UTF_8))
        md.update(language.toByteArray(Charsets.UTF_8))

        val digest = md.digest()
        val sha256Hex = digest.joinToString("") { "%02x".format(it) }
        val durationMs = System.currentTimeMillis() - tStart
        val audioSec = samplesCount.toDouble() / 16000.0
        val tokens = (audioSec * 3.5).roundToLong()

        WhisperResult(
            sha256Hex = sha256Hex,
            durationMs = durationMs,
            totalBytes = samplesCount * 2,
            tokensGenerated = tokens,
            audioSeconds = audioSec
        )
    }
}
