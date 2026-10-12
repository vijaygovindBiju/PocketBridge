package com.pocketbridge.worker.profiler

import android.app.ActivityManager
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.Build
import android.os.PowerManager
import com.google.gson.annotations.SerializedName

data class DeviceProfile(
    @SerializedName("role") val role: String = "worker",
    @SerializedName("os") val os: String = "Android",
    @SerializedName("os_version") val osVersion: String,
    @SerializedName("architecture") val architecture: String,
    @SerializedName("cpu_model") val cpuModel: String,
    @SerializedName("cpu_cores") val cpuCores: Int,
    @SerializedName("ram_total_bytes") val ramTotalBytes: Long,
    @SerializedName("ram_available_bytes") val ramAvailableBytes: Long,
    @SerializedName("device_model") val deviceModel: String,
    @SerializedName("soc") val soc: String?,
    @SerializedName("android_api_level") val androidApiLevel: Int,
    @SerializedName("battery_percent") val batteryPercent: Int?,
    @SerializedName("battery_charging") val batteryCharging: Boolean?,
    @SerializedName("thermal_status") val thermalStatus: String?
)

object DeviceProfiler {
    fun collect(context: Context): DeviceProfile {
        val am = context.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
        val memInfo = ActivityManager.MemoryInfo()
        am?.getMemoryInfo(memInfo)

        val (batteryPct, isCharging) = getBatteryInfo(context)
        val thermal = getThermalStatus(context)

        val soc = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            "${Build.SOC_MANUFACTURER} ${Build.SOC_MODEL}".trim()
        } else {
            Build.HARDWARE
        }

        return DeviceProfile(
            role = "worker",
            os = "Android",
            osVersion = "Android ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT})",
            architecture = Build.SUPPORTED_ABIS.firstOrNull() ?: "aarch64",
            cpuModel = soc,
            cpuCores = Runtime.getRuntime().availableProcessors(),
            ramTotalBytes = memInfo.totalMem,
            ramAvailableBytes = memInfo.availMem,
            deviceModel = "${Build.MANUFACTURER} ${Build.MODEL}",
            soc = soc,
            androidApiLevel = Build.VERSION.SDK_INT,
            batteryPercent = batteryPct,
            batteryCharging = isCharging,
            thermalStatus = thermal
        )
    }

    private fun getBatteryInfo(context: Context): Pair<Int?, Boolean?> {
        val filter = IntentFilter(Intent.ACTION_BATTERY_CHANGED)
        val batteryStatus: Intent? = context.registerReceiver(null, filter)
        if (batteryStatus == null) return Pair(null, null)

        val level = batteryStatus.getIntExtra(BatteryManager.EXTRA_LEVEL, -1)
        val scale = batteryStatus.getIntExtra(BatteryManager.EXTRA_SCALE, -1)
        val pct = if (level >= 0 && scale > 0) (level * 100 / scale) else null

        val status = batteryStatus.getIntExtra(BatteryManager.EXTRA_STATUS, -1)
        val isCharging = status == BatteryManager.BATTERY_STATUS_CHARGING ||
                status == BatteryManager.BATTERY_STATUS_FULL

        return Pair(pct, isCharging)
    }

    private fun getThermalStatus(context: Context): String? {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val pm = context.getSystemService(Context.POWER_SERVICE) as? PowerManager ?: return null
            return when (pm.currentThermalStatus) {
                PowerManager.THERMAL_STATUS_NONE -> "none"
                PowerManager.THERMAL_STATUS_LIGHT -> "light"
                PowerManager.THERMAL_STATUS_MODERATE -> "moderate"
                PowerManager.THERMAL_STATUS_SEVERE -> "severe"
                PowerManager.THERMAL_STATUS_CRITICAL -> "critical"
                PowerManager.THERMAL_STATUS_EMERGENCY -> "emergency"
                PowerManager.THERMAL_STATUS_SHUTDOWN -> "shutdown"
                else -> "unknown"
            }
        }
        return null
    }
}
