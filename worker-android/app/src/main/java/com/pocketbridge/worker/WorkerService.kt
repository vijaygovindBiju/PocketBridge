package com.pocketbridge.worker

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import com.google.gson.Gson
import com.pocketbridge.worker.profiler.DeviceProfiler
import com.pocketbridge.worker.profiler.ThermalMonitor
import com.pocketbridge.worker.protocol.BenchmarkEchoPayload
import com.pocketbridge.worker.protocol.BenchmarkEchoReplyPayload
import com.pocketbridge.worker.protocol.CancelJobPayload
import com.pocketbridge.worker.protocol.ChunkTransformParams
import com.pocketbridge.worker.protocol.Envelope
import com.pocketbridge.worker.protocol.FrameCodec
import com.pocketbridge.worker.protocol.HeartbeatPayload
import com.pocketbridge.worker.protocol.HelloPayload
import com.pocketbridge.worker.protocol.JobAcceptedPayload
import com.pocketbridge.worker.protocol.DataAnalyticsParams
import com.pocketbridge.worker.protocol.ImageBatchParams
import com.pocketbridge.worker.protocol.JobCancelledPayload
import com.pocketbridge.worker.protocol.JobErrorPayload
import com.pocketbridge.worker.protocol.JobProgressPayload
import com.pocketbridge.worker.protocol.JobRejectedPayload
import com.pocketbridge.worker.protocol.JobStartedPayload
import com.pocketbridge.worker.protocol.MessageKind
import com.pocketbridge.worker.protocol.ResultManifestPayload
import com.pocketbridge.worker.protocol.SubmitJobPayload
import com.pocketbridge.worker.receiver.PowerStateReceiver
import com.pocketbridge.worker.runtime.ChunkTransformEngine
import com.pocketbridge.worker.runtime.DataAnalyticsEngine
import com.pocketbridge.worker.runtime.ImageBatchEngine
import com.pocketbridge.worker.runtime.TaskRuntime
import com.pocketbridge.worker.service.WakeLockManager
import com.pocketbridge.worker.service.WorkerNotification
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.io.BufferedInputStream
import java.io.BufferedOutputStream
import java.io.DataInputStream
import java.io.DataOutputStream
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.util.UUID

class WorkerService : Service() {

    companion object {
        private const val TAG = "WorkerService"
        const val PORT = 9876
        const val CHANNEL_ID = "pocketbridge_worker_channel"
        const val NOTIFICATION_ID = 1001

        const val ACTION_START = "com.pocketbridge.worker.START"
        const val ACTION_STOP = "com.pocketbridge.worker.STOP"

        private val _isRunning = MutableStateFlow(false)
        val isRunning: StateFlow<Boolean> = _isRunning

        private val _statusText = MutableStateFlow("Stopped")
        val statusText: StateFlow<String> = _statusText

        private val _recentLog = MutableStateFlow<List<String>>(emptyList())
        val recentLog: StateFlow<List<String>> = _recentLog

        fun log(msg: String) {
            Log.i(TAG, msg)
            val current = _recentLog.value.toMutableList()
            current.add(msg)
            if (current.size > 50) current.removeAt(0)
            _recentLog.value = current
        }
    }

    private val serviceScope = CoroutineScope(Dispatchers.IO + SupervisorJob())
    private val gson = Gson()
    private val taskRuntime = TaskRuntime()
    private var serverSocket: ServerSocket? = null
    private val startTimeMs = System.currentTimeMillis()

    private lateinit var wakeLockManager: WakeLockManager
    private lateinit var thermalMonitor: ThermalMonitor
    private val powerStateReceiver = PowerStateReceiver()

    override fun onCreate() {
        super.onCreate()
        wakeLockManager = WakeLockManager(this)
        thermalMonitor = ThermalMonitor(this).apply { start() }
        powerStateReceiver.register(this)
        WorkerNotification.createNotificationChannel(this)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            stopWorker()
            stopSelf()
            return START_NOT_STICKY
        }

        startForegroundServiceNotification()
        startTcpServer()
        return START_STICKY
    }

    override fun onDestroy() {
        stopWorker()
        powerStateReceiver.unregister(this)
        thermalMonitor.stop()
        wakeLockManager.releaseAll()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun startForegroundServiceNotification() {
        val notification = WorkerNotification.buildIdleNotification(this, "Listening on 127.0.0.1:$PORT")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE
            } else {
                0
            }
            startForeground(WorkerNotification.NOTIFICATION_ID, notification, type)
        } else {
            startForeground(WorkerNotification.NOTIFICATION_ID, notification)
        }
        _isRunning.value = true
        _statusText.value = "Listening on 127.0.0.1:$PORT"
        log("Foreground service started on port $PORT (Power & Thermal monitors active)")
    }

    private fun startTcpServer() {
        serviceScope.launch {
            try {
                serverSocket = ServerSocket(PORT, 50, InetAddress.getByName("127.0.0.1"))
                log("TCP Server listening on 127.0.0.1:$PORT")

                while (isActive) {
                    val clientSocket = serverSocket?.accept() ?: break
                    log("Accepted client connection from ${clientSocket.remoteSocketAddress}")
                    launch { handleClient(clientSocket) }
                }
            } catch (e: Exception) {
                if (isActive) {
                    log("Server socket error: ${e.message}")
                }
            }
        }
    }

    private suspend fun handleClient(socket: Socket) {
        val rawInput = socket.getInputStream()
        val rawOutput = socket.getOutputStream()
        val dataIn = DataInputStream(BufferedInputStream(rawInput, 64 * 1024))
        val dataOut = DataOutputStream(BufferedOutputStream(rawOutput, 64 * 1024))

        try {
            while (serviceScope.isActive) {
                val envelope = FrameCodec.readEnvelope(dataIn, gson)
                val rxTime = System.currentTimeMillis()

                when (envelope.kind) {
                    MessageKind.HELLO -> {
                        log("Received HELLO handshake from host")
                        val helloReply = HelloPayload(
                            agentName = "pocketbridge-android",
                            agentVersion = "0.1.0",
                            protocolVersionMin = 1,
                            protocolVersionMax = 1,
                            role = "worker"
                        )
                        val replyEnv = Envelope(
                            kind = MessageKind.HELLO,
                            payload = gson.toJsonTree(helloReply)
                        )
                        FrameCodec.writeEnvelope(dataOut, replyEnv, gson)
                    }

                    MessageKind.HEARTBEAT -> {
                        val profile = DeviceProfiler.collect(this@WorkerService)
                        val uptime = System.currentTimeMillis() - startTimeMs
                        val hbReply = HeartbeatPayload(
                            uptimeMs = uptime,
                            memoryUsedBytes = profile.ramTotalBytes - profile.ramAvailableBytes,
                            memoryTotalBytes = profile.ramTotalBytes,
                            batteryPercent = PowerStateReceiver.batteryPercent.value ?: profile.batteryPercent,
                            batteryCharging = PowerStateReceiver.isCharging.value ?: profile.batteryCharging,
                            temperatureCelsius = PowerStateReceiver.temperatureCelsius.value?.toDouble(),
                            thermalStatus = thermalMonitor.statusString.value
                        )
                        val replyEnv = Envelope(
                            kind = MessageKind.HEARTBEAT,
                            payload = gson.toJsonTree(hbReply)
                        )
                        FrameCodec.writeEnvelope(dataOut, replyEnv, gson)
                    }

                    MessageKind.BENCHMARK_ECHO -> {
                        val echoPayload = gson.fromJson(envelope.payload, BenchmarkEchoPayload::class.java)
                        val bytesToEcho = echoPayload.binarySizeBytes

                        val txTime = System.currentTimeMillis()
                        val echoReply = BenchmarkEchoReplyPayload(
                            sequence = echoPayload.sequence,
                            binarySizeBytes = bytesToEcho,
                            originalSentAtUnixMs = echoPayload.sentAtUnixMs,
                            workerReceivedAtUnixMs = rxTime,
                            workerRepliedAtUnixMs = txTime
                        )
                        val replyEnv = Envelope(
                            kind = MessageKind.BENCHMARK_ECHO_REPLY,
                            payload = gson.toJsonTree(echoReply)
                        )
                        FrameCodec.writeEnvelope(dataOut, replyEnv, gson)

                        if (bytesToEcho > 0) {
                            FrameCodec.streamEcho(dataIn, dataOut, bytesToEcho)
                        }
                    }

                    MessageKind.SUBMIT_JOB -> {
                        val submit = gson.fromJson(envelope.payload, SubmitJobPayload::class.java)
                        log("Received SUBMIT_JOB: id=${submit.jobId}, adapter=${submit.adapterId}")

                        when (submit.adapterId) {
                            ChunkTransformEngine.ADAPTER_ID -> {
                                val attemptId = UUID.randomUUID().toString()
                                val accept = JobAcceptedPayload(submit.jobId, attemptId)
                                val acceptEnv = Envelope(kind = MessageKind.JOB_ACCEPTED, payload = gson.toJsonTree(accept))
                                FrameCodec.writeEnvelope(dataOut, acceptEnv, gson)

                                val started = JobStartedPayload(submit.jobId)
                                val startedEnv = Envelope(kind = MessageKind.JOB_STARTED, payload = gson.toJsonTree(started))
                                FrameCodec.writeEnvelope(dataOut, startedEnv, gson)

                                val params = gson.fromJson(submit.inputParameters, ChunkTransformParams::class.java)

                                 val job = serviceScope.launch {
                                    wakeLockManager.acquire(submit.jobId)
                                    try {
                                        val (sha256Hex, computeMs) = ChunkTransformEngine.execute(
                                            recordCount = params.recordCount,
                                            rounds = params.rounds,
                                            seed = params.seed,
                                            onProgress = { fraction ->
                                                val elapsed = System.currentTimeMillis() - accept.acceptedAtUnixMs
                                                WorkerNotification.updateProgressNotification(
                                                    this@WorkerService,
                                                    submit.adapterId,
                                                    fraction,
                                                    elapsed,
                                                    thermalMonitor.statusString.value,
                                                    PowerStateReceiver.batteryPercent.value
                                                )
                                                val prog = JobProgressPayload(
                                                    jobId = submit.jobId,
                                                    progressFraction = fraction,
                                                    elapsedComputeMs = elapsed
                                                )
                                                val progEnv = Envelope(kind = MessageKind.JOB_PROGRESS, payload = gson.toJsonTree(prog))
                                                synchronized(dataOut) {
                                                    FrameCodec.writeEnvelope(dataOut, progEnv, gson)
                                                }
                                            }
                                        )

                                        val manifest = ResultManifestPayload(
                                            jobId = submit.jobId,
                                            adapterId = submit.adapterId,
                                            status = "succeeded",
                                            outputSizeBytes = params.recordCount * 8,
                                            outputSha256 = sha256Hex,
                                            computeDurationMs = computeMs,
                                            recordsProcessed = params.recordCount
                                        )
                                        val manifestEnv = Envelope(kind = MessageKind.RESULT_MANIFEST, payload = gson.toJsonTree(manifest))
                                        synchronized(dataOut) {
                                            FrameCodec.writeEnvelope(dataOut, manifestEnv, gson)
                                        }
                                        log("Job ${submit.jobId} (chunk_transform) finished in ${computeMs}ms (sha=$sha256Hex)")
                                    } catch (e: CancellationException) {
                                        log("Job ${submit.jobId} cancelled")
                                        val cancelEnv = Envelope(kind = MessageKind.CANCELLED, payload = gson.toJsonTree(JobCancelledPayload(submit.jobId)))
                                        synchronized(dataOut) {
                                            FrameCodec.writeEnvelope(dataOut, cancelEnv, gson)
                                        }
                                    } catch (e: Exception) {
                                        log("Job ${submit.jobId} failed: ${e.message}")
                                        val err = JobErrorPayload(submit.jobId, "EXECUTION_ERROR", e.message ?: "Unknown error")
                                        val errEnv = Envelope(kind = MessageKind.JOB_ERROR, payload = gson.toJsonTree(err))
                                        synchronized(dataOut) {
                                            FrameCodec.writeEnvelope(dataOut, errEnv, gson)
                                        }
                                    } finally {
                                        wakeLockManager.release(submit.jobId)
                                        WorkerNotification.updateIdle(this@WorkerService, "Listening on 127.0.0.1:$PORT")
                                        taskRuntime.unregisterJob(submit.jobId)
                                    }
                                }
                                taskRuntime.registerJob(submit.jobId, job)
                            }

                            ImageBatchEngine.ADAPTER_ID -> {
                                val attemptId = UUID.randomUUID().toString()
                                val accept = JobAcceptedPayload(submit.jobId, attemptId)
                                val acceptEnv = Envelope(kind = MessageKind.JOB_ACCEPTED, payload = gson.toJsonTree(accept))
                                FrameCodec.writeEnvelope(dataOut, acceptEnv, gson)

                                val started = JobStartedPayload(submit.jobId)
                                val startedEnv = Envelope(kind = MessageKind.JOB_STARTED, payload = gson.toJsonTree(started))
                                FrameCodec.writeEnvelope(dataOut, startedEnv, gson)

                                val params = gson.fromJson(submit.inputParameters, ImageBatchParams::class.java)

                                 val job = serviceScope.launch {
                                    wakeLockManager.acquire(submit.jobId)
                                    try {
                                        val (sha256Hex, computeMs, totalBytes) = ImageBatchEngine.execute(
                                            imageCount = params.imageCount,
                                            width = params.width,
                                            height = params.height,
                                            seed = params.seed,
                                            operation = params.operation,
                                            onProgress = { fraction ->
                                                val elapsed = System.currentTimeMillis() - accept.acceptedAtUnixMs
                                                WorkerNotification.updateProgressNotification(
                                                    this@WorkerService,
                                                    submit.adapterId,
                                                    fraction,
                                                    elapsed,
                                                    thermalMonitor.statusString.value,
                                                    PowerStateReceiver.batteryPercent.value
                                                )
                                                val prog = JobProgressPayload(
                                                    jobId = submit.jobId,
                                                    progressFraction = fraction,
                                                    elapsedComputeMs = elapsed
                                                )
                                                val progEnv = Envelope(kind = MessageKind.JOB_PROGRESS, payload = gson.toJsonTree(prog))
                                                synchronized(dataOut) {
                                                    FrameCodec.writeEnvelope(dataOut, progEnv, gson)
                                                }
                                            }
                                        )

                                        val manifest = ResultManifestPayload(
                                            jobId = submit.jobId,
                                            adapterId = submit.adapterId,
                                            status = "succeeded",
                                            outputSizeBytes = totalBytes,
                                            outputSha256 = sha256Hex,
                                            computeDurationMs = computeMs,
                                            recordsProcessed = params.imageCount.toLong()
                                        )
                                        val manifestEnv = Envelope(kind = MessageKind.RESULT_MANIFEST, payload = gson.toJsonTree(manifest))
                                        synchronized(dataOut) {
                                            FrameCodec.writeEnvelope(dataOut, manifestEnv, gson)
                                        }
                                        log("Job ${submit.jobId} (image_batch) finished in ${computeMs}ms (sha=$sha256Hex)")
                                    } catch (e: CancellationException) {
                                        log("Job ${submit.jobId} cancelled")
                                        val cancelEnv = Envelope(kind = MessageKind.CANCELLED, payload = gson.toJsonTree(JobCancelledPayload(submit.jobId)))
                                        synchronized(dataOut) {
                                            FrameCodec.writeEnvelope(dataOut, cancelEnv, gson)
                                        }
                                    } catch (e: Exception) {
                                        log("Job ${submit.jobId} failed: ${e.message}")
                                        val err = JobErrorPayload(submit.jobId, "EXECUTION_ERROR", e.message ?: "Unknown error")
                                        val errEnv = Envelope(kind = MessageKind.JOB_ERROR, payload = gson.toJsonTree(err))
                                        synchronized(dataOut) {
                                            FrameCodec.writeEnvelope(dataOut, errEnv, gson)
                                        }
                                    } finally {
                                        wakeLockManager.release(submit.jobId)
                                        WorkerNotification.updateIdle(this@WorkerService, "Listening on 127.0.0.1:$PORT")
                                        taskRuntime.unregisterJob(submit.jobId)
                                    }
                                }
                                taskRuntime.registerJob(submit.jobId, job)
                            }

                            DataAnalyticsEngine.ADAPTER_ID -> {
                                val attemptId = UUID.randomUUID().toString()
                                val accept = JobAcceptedPayload(submit.jobId, attemptId)
                                val acceptEnv = Envelope(kind = MessageKind.JOB_ACCEPTED, payload = gson.toJsonTree(accept))
                                FrameCodec.writeEnvelope(dataOut, acceptEnv, gson)

                                val started = JobStartedPayload(submit.jobId)
                                val startedEnv = Envelope(kind = MessageKind.JOB_STARTED, payload = gson.toJsonTree(started))
                                FrameCodec.writeEnvelope(dataOut, startedEnv, gson)

                                val params = gson.fromJson(submit.inputParameters, DataAnalyticsParams::class.java)

                                 val job = serviceScope.launch {
                                    wakeLockManager.acquire(submit.jobId)
                                    try {
                                        val result = DataAnalyticsEngine.execute(
                                            recordCount = params.recordCount,
                                            seed = params.seed,
                                            filterMin = params.filterMin,
                                            filterMax = params.filterMax,
                                            onProgress = { fraction ->
                                                val elapsed = System.currentTimeMillis() - accept.acceptedAtUnixMs
                                                WorkerNotification.updateProgressNotification(
                                                    this@WorkerService,
                                                    submit.adapterId,
                                                    fraction,
                                                    elapsed,
                                                    thermalMonitor.statusString.value,
                                                    PowerStateReceiver.batteryPercent.value
                                                )
                                                val prog = JobProgressPayload(
                                                    jobId = submit.jobId,
                                                    progressFraction = fraction,
                                                    elapsedComputeMs = elapsed
                                                )
                                                val progEnv = Envelope(kind = MessageKind.JOB_PROGRESS, payload = gson.toJsonTree(prog))
                                                synchronized(dataOut) {
                                                    FrameCodec.writeEnvelope(dataOut, progEnv, gson)
                                                }
                                            }
                                        )

                                        val manifest = ResultManifestPayload(
                                            jobId = submit.jobId,
                                            adapterId = submit.adapterId,
                                            status = "succeeded",
                                            outputSizeBytes = result.totalPassed * 8,
                                            outputSha256 = result.sha256Hex,
                                            computeDurationMs = result.durationMs,
                                            recordsProcessed = result.totalPassed
                                        )
                                        val manifestEnv = Envelope(kind = MessageKind.RESULT_MANIFEST, payload = gson.toJsonTree(manifest))
                                        synchronized(dataOut) {
                                            FrameCodec.writeEnvelope(dataOut, manifestEnv, gson)
                                        }
                                        log("Job ${submit.jobId} (data_analytics) finished in ${result.durationMs}ms (sha=${result.sha256Hex})")
                                    } catch (e: CancellationException) {
                                        log("Job ${submit.jobId} cancelled")
                                        val cancelEnv = Envelope(kind = MessageKind.CANCELLED, payload = gson.toJsonTree(JobCancelledPayload(submit.jobId)))
                                        synchronized(dataOut) {
                                            FrameCodec.writeEnvelope(dataOut, cancelEnv, gson)
                                        }
                                    } catch (e: Exception) {
                                        log("Job ${submit.jobId} failed: ${e.message}")
                                        val err = JobErrorPayload(submit.jobId, "EXECUTION_ERROR", e.message ?: "Unknown error")
                                        val errEnv = Envelope(kind = MessageKind.JOB_ERROR, payload = gson.toJsonTree(err))
                                        synchronized(dataOut) {
                                            FrameCodec.writeEnvelope(dataOut, errEnv, gson)
                                        }
                                    } finally {
                                        wakeLockManager.release(submit.jobId)
                                        WorkerNotification.updateIdle(this@WorkerService, "Listening on 127.0.0.1:$PORT")
                                        taskRuntime.unregisterJob(submit.jobId)
                                    }
                                }
                                taskRuntime.registerJob(submit.jobId, job)
                            }

                            else -> {
                                val rej = JobRejectedPayload(submit.jobId, "Unsupported adapter: ${submit.adapterId}", "UNSUPPORTED_ADAPTER")
                                val rejEnv = Envelope(kind = MessageKind.JOB_REJECTED, payload = gson.toJsonTree(rej))
                                FrameCodec.writeEnvelope(dataOut, rejEnv, gson)
                            }
                        }
                    }

                    MessageKind.CANCEL_JOB -> {
                        val cancelPayload = gson.fromJson(envelope.payload, CancelJobPayload::class.java)
                        log("Received CANCEL_JOB for ${cancelPayload.jobId}")
                        taskRuntime.cancelJob(cancelPayload.jobId)
                    }

                    MessageKind.GOODBYE -> {
                        log("Client sent GOODBYE. Closing connection.")
                        break
                    }

                    else -> {
                        log("Unknown envelope kind: ${envelope.kind}")
                    }
                }
            }
        } catch (e: Exception) {
            log("Client session ended: ${e.message}")
        } finally {
            try { socket.close() } catch (_: Exception) {}
        }
    }

    private fun stopWorker() {
        _isRunning.value = false
        _statusText.value = "Stopped"
        log("Stopping worker service...")
        try { serverSocket?.close() } catch (_: Exception) {}
        serviceScope.cancel()
        WorkerNotification.updateIdle(this, "Worker Stopped")
    }
}
