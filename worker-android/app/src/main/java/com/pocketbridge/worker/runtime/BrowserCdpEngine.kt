package com.pocketbridge.worker.runtime

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlin.math.max

object BrowserCdpEngine {
    const val ADAPTER_ID = "browser_cdp.v1"

    data class BrowserCdpResult(
        val sha256Hex: String,
        val durationMs: Long,
        val totalBytes: Long,
        val backgroundTabs: Int,
        val ramSavedMb: Long
    )

    suspend fun execute(
        totalTabs: Int,
        activeTabs: Int,
        onProgress: suspend (Float) -> Unit
    ): BrowserCdpResult = coroutineScope {
        val tStart = System.currentTimeMillis()
        val bgTabs = max(0, totalTabs - activeTabs)
        val ramSaved = bgTabs.toLong() * 150L // 150 MB average per tab

        // Simulate headless tab DOM serialization & background suspension
        val deferredTabs = (0 until bgTabs).map { tabIdx ->
            async(Dispatchers.Default) {
                // Simulate parsing/rendering 2KB HTML DOM
                var checksum = 0L
                for (b in 0 until 2048) {
                    checksum = (checksum * 31 + (tabIdx xor b)) and 0x7FFFFFFF
                }
                checksum
            }
        }

        val reportStep = max(1, bgTabs / 10)
        var done = 0
        for (deferred in deferredTabs) {
            deferred.await()
            done++
            if (done % reportStep == 0 || done == bgTabs) {
                onProgress(done.toFloat() / max(1, bgTabs))
            }
        }

        // Compute deterministic SHA-256 matching dynamic_adapter.rs
        val md = MessageDigest.getInstance("SHA-256")
        md.update("BROWSER_CDP_RENDER_V1:".toByteArray(Charsets.UTF_8))
        val buf1 = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(totalTabs.toLong())
        md.update(buf1.array())
        val buf2 = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(bgTabs.toLong())
        md.update(buf2.array())

        val digest = md.digest()
        val sha256Hex = digest.joinToString("") { "%02x".format(it) }
        val durationMs = System.currentTimeMillis() - tStart

        BrowserCdpResult(
            sha256Hex = sha256Hex,
            durationMs = durationMs,
            totalBytes = totalTabs.toLong() * 2048L,
            backgroundTabs = bgTabs,
            ramSavedMb = ramSaved
        )
    }
}
