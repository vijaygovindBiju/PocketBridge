package com.pocketbridge.worker.protocol

import com.google.gson.Gson
import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.InputStream
import java.io.OutputStream
import kotlin.math.min

object FrameCodec {
    private const val MAX_ENVELOPE_SIZE = 4 * 1024 * 1024 // 4 MB
    private const val CHUNK_SIZE = 64 * 1024 // 64 KB

    fun writeEnvelope(out: DataOutputStream, envelope: Envelope, gson: Gson) {
        val jsonBytes = gson.toJson(envelope).toByteArray(Charsets.UTF_8)
        if (jsonBytes.size > MAX_ENVELOPE_SIZE) {
            throw IllegalArgumentException("Envelope exceeds maximum size: ${jsonBytes.size}")
        }
        out.writeInt(jsonBytes.size)
        out.write(jsonBytes)
        out.flush()
    }

    fun readEnvelope(input: DataInputStream, gson: Gson): Envelope {
        val length = input.readInt()
        if (length < 0 || length > MAX_ENVELOPE_SIZE) {
            throw IllegalArgumentException("Invalid envelope length: $length")
        }
        val buffer = ByteArray(length)
        input.readFully(buffer)
        val jsonStr = String(buffer, Charsets.UTF_8)
        return gson.fromJson(jsonStr, Envelope::class.java)
    }

    /**
     * Reads [totalBytes] from [input] and immediately echoes them to [output] using a 64KB buffer.
     * Keeps memory consumption O(1) regardless of payload size (1MB, 10MB, 100MB).
     */
    fun streamEcho(input: InputStream, output: OutputStream, totalBytes: Long) {
        val buffer = ByteArray(CHUNK_SIZE)
        var remaining = totalBytes
        while (remaining > 0) {
            val toRead = min(remaining, CHUNK_SIZE.toLong()).toInt()
            var readTotal = 0
            while (readTotal < toRead) {
                val n = input.read(buffer, readTotal, toRead - readTotal)
                if (n == -1) {
                    throw IllegalStateException("Unexpected EOF while streaming echo. Remaining: $remaining")
                }
                readTotal += n
            }
            output.write(buffer, 0, toRead)
            remaining -= toRead
        }
        output.flush()
    }
}
