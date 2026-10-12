package com.pocketbridge.worker.profiler

import android.content.Context
import android.os.Build
import android.os.PowerManager
import android.util.Log
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/**
 * Monitors Android OS hardware thermal status in real-time (Android 10+ / API 29+).
 * Dynamically adjusts worker concurrency during thermal throttling to protect
 * battery health and device temperatures.
 */
class ThermalMonitor(private val context: Context) {

    companion object {
        private const val TAG = "ThermalMonitor"

        fun thermalStatusToString(status: Int): String {
            return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                when (status) {
                    PowerManager.THERMAL_STATUS_NONE -> "none"
                    PowerManager.THERMAL_STATUS_LIGHT -> "light"
                    PowerManager.THERMAL_STATUS_MODERATE -> "moderate"
                    PowerManager.THERMAL_STATUS_SEVERE -> "severe"
                    PowerManager.THERMAL_STATUS_CRITICAL -> "critical"
                    PowerManager.THERMAL_STATUS_EMERGENCY -> "emergency"
                    PowerManager.THERMAL_STATUS_SHUTDOWN -> "shutdown"
                    else -> "unknown ($status)"
                }
            } else {
                "unsupported"
            }
        }
    }

    private val powerManager = context.getSystemService(Context.POWER_SERVICE) as? PowerManager

    private val _statusString = MutableStateFlow(queryCurrentStatus())
    val statusString: StateFlow<String> = _statusString

    private val _statusInt = MutableStateFlow(queryCurrentStatusInt())
    val statusInt: StateFlow<Int> = _statusInt

    private var thermalListener: PowerManager.OnThermalStatusChangedListener? = null

    fun start() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && powerManager != null) {
            val listener = PowerManager.OnThermalStatusChangedListener { status ->
                val str = thermalStatusToString(status)
                _statusInt.value = status
                _statusString.value = str
                Log.w(TAG, "Thermal status changed: $str ($status)")
            }
            thermalListener = listener
            try {
                powerManager.addThermalStatusListener(context.mainExecutor, listener)
                Log.i(TAG, "Registered OnThermalStatusChangedListener (Current: ${_statusString.value})")
            } catch (e: Exception) {
                Log.e(TAG, "Failed to register thermal status listener: ${e.message}")
            }
        }
    }

    fun stop() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && powerManager != null) {
            thermalListener?.let { listener ->
                try {
                    powerManager.removeThermalStatusListener(listener)
                    Log.i(TAG, "Unregistered OnThermalStatusChangedListener")
                } catch (e: Exception) {
                    Log.e(TAG, "Failed to remove thermal status listener: ${e.message}")
                }
            }
        }
        thermalListener = null
    }

    fun queryCurrentStatus(): String {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && powerManager != null) {
            try {
                thermalStatusToString(powerManager.currentThermalStatus)
            } catch (e: Exception) {
                "unknown"
            }
        } else {
            "unsupported"
        }
    }

    fun queryCurrentStatusInt(): Int {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && powerManager != null) {
            try {
                powerManager.currentThermalStatus
            } catch (e: Exception) {
                0
            }
        } else {
            0
        }
    }

    /**
     * Compute safe core concurrency given the current thermal conditions.
     */
    fun recommendedConcurrency(baseCores: Int): Int {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return baseCores

        return when (_statusInt.value) {
            PowerManager.THERMAL_STATUS_NONE, PowerManager.THERMAL_STATUS_LIGHT -> baseCores
            PowerManager.THERMAL_STATUS_MODERATE -> maxOf(1, baseCores / 2)
            PowerManager.THERMAL_STATUS_SEVERE -> maxOf(1, baseCores / 4)
            PowerManager.THERMAL_STATUS_CRITICAL,
            PowerManager.THERMAL_STATUS_EMERGENCY,
            PowerManager.THERMAL_STATUS_SHUTDOWN -> 1
            else -> baseCores
        }
    }
}
