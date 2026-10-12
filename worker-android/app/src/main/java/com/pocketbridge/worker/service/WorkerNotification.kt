package com.pocketbridge.worker.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.NotificationCompat
import com.pocketbridge.worker.MainActivity

/**
 * Manages foreground service notifications with dynamic real-time progress indicators,
 * elapsed execution duration, thermal status, and battery telemetry.
 */
object WorkerNotification {

    const val CHANNEL_ID = "pocketbridge_worker_channel"
    const val NOTIFICATION_ID = 1001

    fun createNotificationChannel(context: Context) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "PocketBridge Worker Service",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Foreground execution and compute status for PocketBridge worker"
                setShowBadge(false)
            }
            val manager = context.getSystemService(NotificationManager::class.java)
            manager?.createNotificationChannel(channel)
        }
    }

    fun buildIdleNotification(context: Context, statusMessage: String): Notification {
        val pendingIntent = createContentIntent(context)
        return NotificationCompat.Builder(context, CHANNEL_ID)
            .setContentTitle("PocketBridge Worker Active")
            .setContentText(statusMessage)
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setOngoing(true)
            .setContentIntent(pendingIntent)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
    }

    fun updateProgressNotification(
        context: Context,
        adapterId: String,
        fraction: Float,
        elapsedMs: Long,
        thermalStatus: String? = null,
        batteryPercent: Int? = null
    ) {
        val manager = context.getSystemService(Context.NOTIFICATION_SERVICE) as? NotificationManager ?: return
        val pendingIntent = createContentIntent(context)

        val percentInt = (fraction * 100).toInt().coerceIn(0, 100)
        val elapsedSec = elapsedMs / 1000.0

        val telemetry = buildString {
            append("Computing: $percentInt% (${String.format("%.1f", elapsedSec)}s)")
            if (batteryPercent != null) {
                append(" • Bat: $batteryPercent%")
            }
            if (thermalStatus != null) {
                append(" • Thermal: $thermalStatus")
            }
        }

        val notification = NotificationCompat.Builder(context, CHANNEL_ID)
            .setContentTitle("PocketBridge: Running $adapterId")
            .setContentText(telemetry)
            .setSmallIcon(android.R.drawable.stat_notify_sync)
            .setOngoing(true)
            .setContentIntent(pendingIntent)
            .setProgress(100, percentInt, false)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setOnlyAlertOnce(true)
            .build()

        manager.notify(NOTIFICATION_ID, notification)
    }

    fun updateIdle(context: Context, statusMessage: String) {
        val manager = context.getSystemService(Context.NOTIFICATION_SERVICE) as? NotificationManager ?: return
        manager.notify(NOTIFICATION_ID, buildIdleNotification(context, statusMessage))
    }

    private fun createContentIntent(context: Context): PendingIntent {
        val intent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        val flags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        } else {
            PendingIntent.FLAG_UPDATE_CURRENT
        }
        return PendingIntent.getActivity(context, 0, intent, flags)
    }
}
