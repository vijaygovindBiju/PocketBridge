package com.pocketbridge.worker.receiver

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.util.Log
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/**
 * Real-time BroadcastReceiver monitoring Android battery state, charging connection,
 * and battery core temperature.
 */
class PowerStateReceiver : BroadcastReceiver() {

    companion object {
        private const val TAG = "PowerStateReceiver"

        private val _batteryPercent = MutableStateFlow<Int?>(null)
        val batteryPercent: StateFlow<Int?> = _batteryPercent

        private val _isCharging = MutableStateFlow<Boolean?>(null)
        val isCharging: StateFlow<Boolean?> = _isCharging

        private val _temperatureCelsius = MutableStateFlow<Float?>(null)
        val temperatureCelsius: StateFlow<Float?> = _temperatureCelsius
    }

    override fun onReceive(context: Context?, intent: Intent?) {
        if (intent == null) return

        when (intent.action) {
            Intent.ACTION_BATTERY_CHANGED -> {
                val level = intent.getIntExtra(BatteryManager.EXTRA_LEVEL, -1)
                val scale = intent.getIntExtra(BatteryManager.EXTRA_SCALE, -1)
                if (level >= 0 && scale > 0) {
                    _batteryPercent.value = ((level.toFloat() / scale.toFloat()) * 100).toInt()
                }

                val status = intent.getIntExtra(BatteryManager.EXTRA_STATUS, -1)
                _isCharging.value = status == BatteryManager.BATTERY_STATUS_CHARGING ||
                        status == BatteryManager.BATTERY_STATUS_FULL

                val tempTenths = intent.getIntExtra(BatteryManager.EXTRA_TEMPERATURE, -1)
                if (tempTenths > 0) {
                    _temperatureCelsius.value = tempTenths / 10.0f
                }
            }

            Intent.ACTION_POWER_CONNECTED -> {
                _isCharging.value = true
                Log.i(TAG, "Charger connected")
            }

            Intent.ACTION_POWER_DISCONNECTED -> {
                _isCharging.value = false
                Log.i(TAG, "Charger disconnected")
            }
        }
    }

    fun register(context: Context) {
        val filter = IntentFilter().apply {
            addAction(Intent.ACTION_BATTERY_CHANGED)
            addAction(Intent.ACTION_POWER_CONNECTED)
            addAction(Intent.ACTION_POWER_DISCONNECTED)
        }
        try {
            val sticky = context.registerReceiver(this, filter)
            sticky?.let { onReceive(context, it) }
            Log.i(TAG, "PowerStateReceiver registered successfully")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to register PowerStateReceiver: ${e.message}")
        }
    }

    fun unregister(context: Context) {
        try {
            context.unregisterReceiver(this)
            Log.i(TAG, "PowerStateReceiver unregistered")
        } catch (e: Exception) {
            Log.e(TAG, "Error unregistering PowerStateReceiver: ${e.message}")
        }
    }
}
