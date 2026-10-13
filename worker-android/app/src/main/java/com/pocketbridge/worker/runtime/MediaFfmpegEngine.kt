package com.pocketbridge.worker.runtime

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlin.math.max

object MediaFfmpegEngine {
    const val ADAPTER_ID = "media_ffmpeg.v1"

    data class FfmpegResult(
        val sha256Hex: String,
        val durationMs: Long,
        val totalBytes: Long,
        val framesProcessed: Long
    )

    suspend fun execute(
        frameCount: Int,
        resolution: String,
        codec: String,
        onProgress: suspend (Float) -> Unit
    ): FfmpegResult = coroutineScope {
        val tStart = System.currentTimeMillis()

        // Slices of frames distributed across mobile CPU cores
        val sliceSize = 30
        val totalSlices = max(1, (frameCount + sliceSize - 1) / sliceSize)

        val deferredSlices = (0 until totalSlices).map { sliceIdx ->
            val start = sliceIdx * sliceSize
            val end = minOf(start + sliceSize, frameCount)
            async(Dispatchers.Default) {
                // Simulate YUV420p macroblock discrete cosine transform + quantization
                var acc = 0L
                for (f in start until end) {
                    acc = (acc * 33 + f) xor 0x55555555
                }
                acc
            }
        }

        val reportStep = max(1, totalSlices / 10)
        var done = 0
        for (deferred in deferredSlices) {
            deferred.await()
            done++
            if (done % reportStep == 0 || done == totalSlices) {
                onProgress(done.toFloat() / totalSlices)
            }
        }

        // Compute deterministic SHA-256 matching dynamic_adapter.rs
        val md = MessageDigest.getInstance("SHA-256")
        md.update("FFMPEG_TRANSCODE_SLICE_V1:".toByteArray(Charsets.UTF_8))
        val buf = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(frameCount.toLong())
        md.update(buf.array())
        md.update(resolution.toByteArray(Charsets.UTF_8))
        md.update(codec.toByteArray(Charsets.UTF_8))

        val digest = md.digest()
        val sha256Hex = digest.joinToString("") { "%02x".format(it) }
        val durationMs = System.currentTimeMillis() - tStart

        // Estimate 20:1 compression from raw 1080p YUV frames
        val rawBytes = frameCount.toLong() * 1920L * 1080L * 3L / 2L
        val compressedBytes = (rawBytes * 0.05).toLong()

        FfmpegResult(
            sha256Hex = sha256Hex,
            durationMs = durationMs,
            totalBytes = compressedBytes,
            framesProcessed = frameCount.toLong()
        )
    }
}
