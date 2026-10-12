package com.pocketbridge.worker.runtime

import java.security.MessageDigest
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlin.math.max
import kotlin.math.min

object ImageBatchEngine {
    const val ADAPTER_ID = "image_batch.v1"

    suspend fun execute(
        imageCount: Int,
        width: Int,
        height: Int,
        seed: Long,
        operation: String,
        onProgress: suspend (Float) -> Unit
    ): Triple<String, Long, Long> = coroutineScope {
        val tStart = System.currentTimeMillis()
        val md = MessageDigest.getInstance("SHA-256")

        val deferredImages = (0 until imageCount).map { imgIdx ->
            async(Dispatchers.Default) {
                processSingleImage(imgIdx, width, height, seed, operation)
            }
        }

        var totalBytes = 0L
        val reportStep = max(1, imageCount / 10)
        var done = 0

        for (deferred in deferredImages) {
            val imgBytes = deferred.await()
            md.update(imgBytes)
            totalBytes += imgBytes.size
            done++

            if (done % reportStep == 0 || done == imageCount) {
                onProgress(done.toFloat() / imageCount)
            }
        }

        val digest = md.digest()
        val sha256Hex = digest.joinToString("") { "%02x".format(it) }
        val durationMs = System.currentTimeMillis() - tStart

        Triple(sha256Hex, durationMs, totalBytes)
    }

    fun processSingleImage(
        imgIdx: Int,
        w: Int,
        h: Int,
        seed: Long,
        operation: String
    ): ByteArray {
        val seed32 = seed.toInt()
        val seedR8 = (seed ushr 8).toInt()
        val seedR16 = (seed ushr 16).toInt()

        // 1. Generate deterministic raw RGB pixels
        val rawPixels = ByteArray(w * h * 3)
        var offset = 0
        for (y in 0 until h) {
            for (x in 0 until w) {
                val r = (((x * 131) xor (y * 197) xor (imgIdx * 283) xor seed32) and 0x7FFFFFFF) % 256
                val g = (((x * 173) xor (y * 151) xor (imgIdx * 311) xor seedR8) and 0x7FFFFFFF) % 256
                val b = (((x * 199) xor (y * 223) xor (imgIdx * 359) xor seedR16) and 0x7FFFFFFF) % 256
                rawPixels[offset] = r.toByte()
                rawPixels[offset + 1] = g.toByte()
                rawPixels[offset + 2] = b.toByte()
                offset += 3
            }
        }

        // 2. Apply requested operation
        return when (operation) {
            "grayscale_invert" -> {
                val out = ByteArray(w * h * 3)
                var outOff = 0
                for (i in 0 until (w * h)) {
                    val idx = i * 3
                    val r = rawPixels[idx].toInt() and 0xFF
                    val g = rawPixels[idx + 1].toInt() and 0xFF
                    val b = rawPixels[idx + 2].toInt() and 0xFF
                    val lum = (299 * r + 587 * g + 114 * b) / 1000
                    val inv = (255 - lum).toByte()
                    out[outOff] = inv
                    out[outOff + 1] = inv
                    out[outOff + 2] = inv
                    outOff += 3
                }
                out
            }
            "downscale_half" -> {
                val wHalf = w / 2
                val hHalf = h / 2
                val out = ByteArray(wHalf * hHalf * 3)
                var outOff = 0
                for (y in 0 until hHalf) {
                    for (x in 0 until wHalf) {
                        var sumR = 0
                        var sumG = 0
                        var sumB = 0
                        for (dy in 0 until 2) {
                            for (dx in 0 until 2) {
                                val srcX = x * 2 + dx
                                val srcY = y * 2 + dy
                                val idx = (srcY * w + srcX) * 3
                                sumR += rawPixels[idx].toInt() and 0xFF
                                sumG += rawPixels[idx + 1].toInt() and 0xFF
                                sumB += rawPixels[idx + 2].toInt() and 0xFF
                            }
                        }
                        out[outOff] = (sumR / 4).toByte()
                        out[outOff + 1] = (sumG / 4).toByte()
                        out[outOff + 2] = (sumB / 4).toByte()
                        outOff += 3
                    }
                }
                out
            }
            "box_blur_3x3" -> {
                val out = ByteArray(w * h * 3)
                var outOff = 0
                for (y in 0 until h) {
                    for (x in 0 until w) {
                        var sumR = 0
                        var sumG = 0
                        var sumB = 0
                        for (dy in -1..1) {
                            for (dx in -1..1) {
                                val px = min(max(x + dx, 0), w - 1)
                                val py = min(max(y + dy, 0), h - 1)
                                val idx = (py * w + px) * 3
                                sumR += rawPixels[idx].toInt() and 0xFF
                                sumG += rawPixels[idx + 1].toInt() and 0xFF
                                sumB += rawPixels[idx + 2].toInt() and 0xFF
                            }
                        }
                        out[outOff] = (sumR / 9).toByte()
                        out[outOff + 1] = (sumG / 9).toByte()
                        out[outOff + 2] = (sumB / 9).toByte()
                        outOff += 3
                    }
                }
                out
            }
            else -> rawPixels
        }
    }
}
