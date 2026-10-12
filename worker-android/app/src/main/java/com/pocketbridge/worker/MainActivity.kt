package com.pocketbridge.worker

import android.content.Intent
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.pocketbridge.worker.profiler.DeviceProfile
import com.pocketbridge.worker.profiler.DeviceProfiler

class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Automatically start the worker service on launch if not already running
        if (!WorkerService.isRunning.value) {
            val startIntent = Intent(this, WorkerService::class.java).apply {
                action = WorkerService.ACTION_START
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                startForegroundService(startIntent)
            } else {
                startService(startIntent)
            }
        }

        setContent {
            MaterialTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    val isRunning by WorkerService.isRunning.collectAsState()
                    val statusText by WorkerService.statusText.collectAsState()
                    val logs by WorkerService.recentLog.collectAsState()
                    val profile = remember { DeviceProfiler.collect(this@MainActivity) }

                    WorkerScreen(
                        isRunning = isRunning,
                        statusText = statusText,
                        profile = profile,
                        logs = logs,
                        onToggleService = {
                            if (isRunning) {
                                val intent = Intent(this, WorkerService::class.java).apply {
                                    action = WorkerService.ACTION_STOP
                                }
                                startService(intent)
                            } else {
                                val intent = Intent(this, WorkerService::class.java).apply {
                                    action = WorkerService.ACTION_START
                                }
                                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                                    startForegroundService(intent)
                                } else {
                                    startService(intent)
                                }
                            }
                        }
                    )
                }
            }
        }
    }
}

@Composable
fun WorkerScreen(
    isRunning: Boolean,
    statusText: String,
    profile: DeviceProfile,
    logs: List<String>,
    onToggleService: () -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp)
    ) {
        // App Title
        Text(
            text = "PocketBridge Worker",
            fontSize = 24.sp,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.primary
        )
        Text(
            text = "Android Compute & Benchmark Agent (v0.1.0)",
            fontSize = 14.sp,
            color = Color.Gray
        )

        Spacer(modifier = Modifier.height(16.dp))

        // Status Card
        Card(
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(12.dp),
            colors = CardDefaults.cardColors(
                containerColor = if (isRunning) Color(0xFFE8F5E9) else Color(0xFFFFEBEE)
            )
        ) {
            Column(modifier = Modifier.padding(16.dp)) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text(
                        text = if (isRunning) "SERVICE RUNNING" else "SERVICE STOPPED",
                        fontWeight = FontWeight.Bold,
                        color = if (isRunning) Color(0xFF2E7D32) else Color(0xFFC62828)
                    )
                    Box(
                        modifier = Modifier
                            .background(
                                color = if (isRunning) Color(0xFF2E7D32) else Color(0xFFC62828),
                                shape = RoundedCornerShape(4.dp)
                            )
                            .padding(horizontal = 8.dp, vertical = 4.dp)
                    ) {
                        Text(
                            text = "Port 9876",
                            color = Color.White,
                            fontSize = 12.sp,
                            fontWeight = FontWeight.Bold
                        )
                    }
                }
                Spacer(modifier = Modifier.height(8.dp))
                Text(text = statusText, fontSize = 14.sp, color = Color.DarkGray)
            }
        }

        Spacer(modifier = Modifier.height(12.dp))

        // Toggle Button
        Button(
            onClick = onToggleService,
            modifier = Modifier.fillMaxWidth(),
            colors = ButtonDefaults.buttonColors(
                containerColor = if (isRunning) Color(0xFFD32F2F) else Color(0xFF1976D2)
            ),
            shape = RoundedCornerShape(8.dp)
        ) {
            Text(
                text = if (isRunning) "Stop Worker Service" else "Start Worker Service",
                fontSize = 16.sp,
                fontWeight = FontWeight.Bold
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        // Device Profile Card
        Card(
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(12.dp),
            colors = CardDefaults.cardColors(containerColor = Color(0xFFF5F5F5))
        ) {
            Column(modifier = Modifier.padding(16.dp)) {
                Text(
                    text = "Device Information",
                    fontWeight = FontWeight.Bold,
                    fontSize = 16.sp
                )
                Spacer(modifier = Modifier.height(8.dp))

                ProfileRow("Device", profile.deviceModel)
                ProfileRow("SoC / CPU", profile.soc ?: profile.cpuModel)
                ProfileRow("Cores", "${profile.cpuCores} cores (${profile.architecture})")
                val totalRamGb = profile.ramTotalBytes / (1024.0 * 1024.0 * 1024.0)
                ProfileRow("Total RAM", String.format("%.2f GB", totalRamGb))
                ProfileRow("OS", profile.osVersion)
                if (profile.batteryPercent != null) {
                    val chargingStr = if (profile.batteryCharging == true) "Charging" else "Discharging"
                    ProfileRow("Battery", "${profile.batteryPercent}% ($chargingStr)")
                }
                if (profile.thermalStatus != null) {
                    ProfileRow("Thermal", profile.thermalStatus)
                }
            }
        }

        Spacer(modifier = Modifier.height(16.dp))

        // Recent Logs
        Text(
            text = "Activity Log",
            fontWeight = FontWeight.Bold,
            fontSize = 16.sp
        )
        Spacer(modifier = Modifier.height(8.dp))

        LazyColumn(
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
                .background(Color(0xFF212121), RoundedCornerShape(8.dp))
                .padding(8.dp)
        ) {
            if (logs.isEmpty()) {
                item {
                    Text(
                        text = "No events yet. Start the worker service to begin.",
                        color = Color.Gray,
                        fontSize = 12.sp
                    )
                }
            } else {
                items(logs.reversed()) { logEntry ->
                    Text(
                        text = logEntry,
                        color = Color(0xFF81C784),
                        fontSize = 12.sp,
                        fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace,
                        modifier = Modifier.padding(vertical = 2.dp)
                    )
                }
            }
        }
    }
}

@Composable
fun ProfileRow(label: String, value: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 2.dp),
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(text = label, color = Color.Gray, fontSize = 13.sp)
        Text(text = value, fontWeight = FontWeight.Medium, fontSize = 13.sp)
    }
}
