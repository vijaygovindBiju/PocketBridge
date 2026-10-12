package com.pocketbridge.worker.service

import android.content.Context
import android.os.PowerManager
import android.util.Log

/**
 * Manages fine-grained Android partial WakeLocks during active compute jobs.
 * Ensures the CPU stays at full performance even if the phone screen turns off,
 * while preventing wakelock leaks via strict timeout guards and lifecycle tracking.
 */
class WakeLockManager(private val context: Context) {

    companion object {
        private const val TAG = "WakeLockManager"
        private const val WAKELOCK_TAG = "PocketBridge:ComputeWakeLock"
        private const val DEFAULT_TIMEOUT_MS = 10 * 60 * 1000L // 10 minutes maximum
    }

    private val powerManager = context.getSystemService(Context.POWER_SERVICE) as? PowerManager
    private var wakeLock: PowerManager.WakeLock? = null
    private val activeJobIds = mutableSetOf<String>()

    init {
        wakeLock = powerManager?.newWakeLock(
            PowerManager.PARTIAL_WAKE_LOCK,
            WAKELOCK_TAG
        )?.apply {
            setReferenceCounted(true)
        }
    }

    @Synchronized
    fun acquire(jobId: String, timeoutMs: Long = DEFAULT_TIMEOUT_MS) {
        val lock = wakeLock ?: run {
            Log.w(TAG, "PowerManager.WakeLock unavailable")
            return
        }

        activeJobIds.add(jobId)
        try {
            lock.acquire(timeoutMs)
            Log.i(TAG, "WakeLock ACQUIRED for job $jobId (Active jobs: ${activeJobIds.size})")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to acquire WakeLock for job $jobId: ${e.message}")
        }
    }

    @Synchronized
    fun release(jobId: String) {
        val lock = wakeLock ?: return
        if (!activeJobIds.remove(jobId)) {
            Log.d(TAG, "Job $jobId was not holding a WakeLock")
            return
        }

        try {
            if (lock.isHeld) {
                lock.release()
                Log.i(TAG, "WakeLock RELEASED for job $jobId (Remaining jobs: ${activeJobIds.size})")
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error releasing WakeLock for job $jobId: ${e.message}")
        }
    }

    @Synchronized
    fun releaseAll() {
        val lock = wakeLock ?: return
        activeJobIds.clear()
        try {
            while (lock.isHeld) {
                lock.release()
            }
            Log.i(TAG, "All WakeLocks forcefully released")
        } catch (e: Exception) {
            Log.e(TAG, "Error in releaseAll: ${e.message}")
        }
    }

    val isHeld: Boolean
        @Synchronized get() = wakeLock?.isHeld == true

    val activeCount: Int
        @Synchronized get() = activeJobIds.size
}
