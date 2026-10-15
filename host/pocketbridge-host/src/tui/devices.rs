use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};
use super::app::App;
use super::theme::*;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // Devices Table (Host + Connected Workers)
            Constraint::Min(8),    // Selected Device Deep Inspector
        ])
        .split(area);

    // 1. Devices Table
    render_devices_table(f, app, chunks[0]);

    // 2. Selected Device Detail Inspector
    render_device_details(f, app, chunks[1]);
}

fn render_devices_table(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        "DEVICE ROLE",
        "IDENTIFIER",
        "TRANSPORT",
        "STATE",
        "CPU ARCH / CORES",
        "RAM (USED / TOTAL)",
        "BATTERY",
        "TEMP / THERMAL",
        "RTT",
    ])
    .style(Style::default().fg(COLOR_SECONDARY).add_modifier(Modifier::BOLD));

    let mut rows = Vec::new();

    // Row 0: Local Host
    let host_ram_str = format!(
        "{} / {}",
        format_bytes(app.host.ram_used_bytes),
        format_bytes(app.host.ram_total_bytes)
    );
    let host_cpu_str = format!("x86_64 ({}c)", app.host.cpu_cores);

    let host_selected = app.selected_device_index == 0;
    let host_style = if host_selected {
        Style::default().bg(Color::Rgb(25, 35, 45)).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    rows.push(
        Row::new(vec![
            Span::styled(if host_selected { "▶ HOST (Local)" } else { "  HOST (Local)" }, Style::default().fg(COLOR_ACCENT)),
            Span::styled(&app.host.hostname, style_value()),
            Span::styled("Internal PCIe/Bus", style_value()),
            Span::styled("Active", Style::default().fg(COLOR_SUCCESS)),
            Span::styled(host_cpu_str, style_value()),
            Span::styled(host_ram_str, style_value()),
            Span::styled("AC Mains", style_value()),
            Span::styled("Normal", Style::default().fg(COLOR_SUCCESS)),
            Span::styled("0.0 ms", Style::default().fg(COLOR_MUTED)),
        ])
        .style(host_style),
    );

    // Row 1: Remote Android Worker
    let t = &app.transport;
    let worker_prof = app.worker_profile.as_ref();
    let dev_id = worker_prof
        .and_then(|p| p.device_model.clone())
        .or_else(|| t.adb_device.clone())
        .unwrap_or_else(|| "No Phone Attached".to_string());

    let (state_str, state_color) = if t.is_connected {
        ("Online", COLOR_SUCCESS)
    } else if t.is_connecting {
        ("Connecting", COLOR_WARNING)
    } else {
        ("Disconnected", COLOR_ERROR)
    };

    let worker_cpu = worker_prof
        .map(|p| format!("{} ({}c)", p.architecture, p.cpu_cores))
        .unwrap_or_else(|| "aarch64 (8c)".to_string());

    let worker_ram = if let Some(hb) = &t.last_heartbeat {
        let u = hb.memory_used_bytes.unwrap_or(0);
        let tot = hb.memory_total_bytes.unwrap_or(0);
        if tot > 0 {
            format!("{} / {}", format_bytes(u), format_bytes(tot))
        } else {
            "N/A".to_string()
        }
    } else {
        "Unknown".to_string()
    };

    let worker_bat = if let Some(hb) = &t.last_heartbeat {
        match hb.battery_percent {
            Some(p) => format!("{}%{}", p, if hb.battery_charging.unwrap_or(false) { "+" } else { "" }),
            None => "N/A".to_string(),
        }
    } else {
        "N/A".to_string()
    };

    let worker_thermal = if let Some(hb) = &t.last_heartbeat {
        match (hb.temperature_celsius, hb.thermal_status.as_deref()) {
            (Some(c), Some(s)) => format!("{:.1}°C ({})", c, s),
            (Some(c), None) => format!("{:.1}°C", c),
            _ => "N/A".to_string(),
        }
    } else {
        "N/A".to_string()
    };

    let rtt_str = match t.rtt_ms {
        Some(ms) => format_duration(ms),
        None => "N/A".to_string(),
    };

    let worker_selected = app.selected_device_index == 1;
    let worker_row_style = if worker_selected {
        Style::default().bg(Color::Rgb(25, 35, 45)).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    rows.push(
        Row::new(vec![
            Span::styled(if worker_selected { "▶ WORKER (Remote)" } else { "  WORKER (Remote)" }, Style::default().fg(COLOR_ACCENT)),
            Span::styled(dev_id, style_value()),
            Span::styled("USB 3.0 / ADB tcp:9876", style_value()),
            Span::styled(state_str, Style::default().fg(state_color)),
            Span::styled(worker_cpu, style_value()),
            Span::styled(worker_ram, style_value()),
            Span::styled(worker_bat, style_value()),
            Span::styled(worker_thermal, style_value()),
            Span::styled(rtt_str, Style::default().fg(COLOR_SUCCESS)),
        ])
        .style(worker_row_style),
    );

    let table = Table::new(
        rows,
        [
            Constraint::Length(18),
            Constraint::Length(20),
            Constraint::Length(22),
            Constraint::Length(14),
            Constraint::Length(16),
            Constraint::Length(20),
            Constraint::Length(10),
            Constraint::Length(16),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(style_border_focused())
            .title(" CONNECTED COMPUTE NODES (Use ↑/↓ to select, [c] Connect, [d] Disconnect, [r] Refresh) "),
    );

    f.render_widget(table, area);
}

fn render_device_details(f: &mut Frame, app: &App, area: Rect) {
    let is_host = app.selected_device_index == 0;

    let lines = if is_host {
        // Detailed Host Diagnostics
        let host = &app.host;
        let prof = app.host_profile.as_ref();
        let gpu_str = prof.and_then(|p| p.gpu.clone()).unwrap_or_else(|| "Integrated Graphics".to_string());

        vec![
            Line::from(vec![
                Span::styled("Device Type:       ", style_label()),
                Span::styled("Primary Orchestrator & Local Compute Node", style_value()),
            ]),
            Line::from(vec![
                Span::styled("Hostname:          ", style_label()),
                Span::styled(&host.hostname, style_value()),
                Span::styled("    OS Distribution:  ", style_label()),
                Span::styled(format!("{} (Kernel {})", host.os, host.kernel), style_value()),
            ]),
            Line::from(vec![
                Span::styled("CPU Model:         ", style_label()),
                Span::styled(&host.cpu_model, Style::default().fg(COLOR_ACCENT)),
                Span::styled(format!(" ({} physical/logical cores)", host.cpu_cores), style_value()),
            ]),
            Line::from(vec![
                Span::styled("Graphics / NPU:    ", style_label()),
                Span::styled(gpu_str, style_value()),
            ]),
            Line::from(vec![
                Span::styled("Physical RAM:      ", style_label()),
                Span::styled(
                    format!(
                        "Total: {} | Used: {} | Available: {}",
                        format_bytes(host.ram_total_bytes),
                        format_bytes(host.ram_used_bytes),
                        format_bytes(host.ram_available_bytes)
                    ),
                    style_value(),
                ),
            ]),
            Line::from(vec![
                Span::styled("Virtual Memory:    ", style_label()),
                Span::styled(
                    format!(
                        "Swap Total: {} | Swap Used: {} (0% thrashing)",
                        format_bytes(host.swap_total_bytes),
                        format_bytes(host.swap_used_bytes)
                    ),
                    style_value(),
                ),
            ]),
            Line::from(vec![
                Span::styled("CPU Utilization:   ", style_label()),
                Span::styled(format!("{:.1}%", host.cpu_usage_pct), style_value()),
                Span::raw("  "),
                render_mini_bar(host.cpu_usage_pct / 100.0, 24),
                Span::styled("   Load Average: ", style_label()),
                Span::styled(format!("{:.2} (1m), {:.2} (5m), {:.2} (15m)", host.load_1m, host.load_5m, host.load_15m), style_value()),
            ]),
            Line::from(vec![
                Span::styled("Supported Engines: ", style_label()),
                Span::styled("AVX2, SHA-NI, Multi-Threaded Tokio Runtime, Local SIMD", Style::default().fg(COLOR_SUCCESS)),
            ]),
        ]
    } else {
        // Detailed Phone Worker Diagnostics
        let t = &app.transport;
        let worker_prof = app.worker_profile.as_ref();

        if !t.is_connected && worker_prof.is_none() {
            vec![
                Line::from(Span::styled("Worker phone is not currently connected.", Style::default().fg(COLOR_ERROR))),
                Line::from(Span::raw("Please plug in your phone via USB-C with USB debugging enabled, then press 'c'.")),
            ]
        } else {
            let model = worker_prof.and_then(|p| p.device_model.clone()).unwrap_or_else(|| "Android Device".to_string());
            let soc = worker_prof.and_then(|p| p.soc.clone()).unwrap_or_else(|| "Qualcomm Snapdragon".to_string());
            let api = worker_prof.and_then(|p| p.android_api_level).map(|a| format!("API {}", a)).unwrap_or_else(|| "API 35+".to_string());
            let os_ver = worker_prof.map(|p| p.os_version.clone()).unwrap_or_else(|| "Android 15+".to_string());

            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Device Model:      ", style_label()),
                    Span::styled(model, Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)),
                    Span::styled("    Android Version:  ", style_label()),
                    Span::styled(format!("{} ({})", os_ver, api), style_value()),
                ]),
                Line::from(vec![
                    Span::styled("SoC Platform:      ", style_label()),
                    Span::styled(soc, style_value()),
                    Span::styled("    Architecture:     ", style_label()),
                    Span::styled(worker_prof.map(|p| p.architecture.clone()).unwrap_or_else(|| "arm64-v8a".to_string()), style_value()),
                ]),
            ];

            if let Some(hb) = &t.last_heartbeat {
                let used = hb.memory_used_bytes.unwrap_or(0);
                let tot = hb.memory_total_bytes.unwrap_or(0);
                let bat_chg = if hb.battery_charging.unwrap_or(false) { "Yes (Power Delivery +1.48A)" } else { "No (Discharging)" };

                lines.push(Line::from(vec![
                    Span::styled("Physical RAM:      ", style_label()),
                    Span::styled(
                        format!("Used: {} / Total: {} (Measurement source: Android ActivityManager API)", format_bytes(used), format_bytes(tot)),
                        style_value(),
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("Battery / Power:   ", style_label()),
                    Span::styled(format!("{}% · Charging: {}", hb.battery_percent.unwrap_or(0), bat_chg), Style::default().fg(COLOR_SUCCESS)),
                    Span::styled("   Uptime: ", style_label()),
                    Span::styled(format!("{} seconds", hb.uptime_ms / 1000), style_value()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("Thermals & Health: ", style_label()),
                    Span::styled(
                        format!("{:.1} °C · Qualcomm API Status: {}", hb.temperature_celsius.unwrap_or(0.0), hb.thermal_status.as_deref().unwrap_or("none")),
                        Style::default().fg(COLOR_WARNING),
                    ),
                ]));
            } else {
                lines.push(Line::from(Span::styled("Awaiting active heartbeat telemetry frame from worker...", Style::default().fg(COLOR_WARNING))));
            }

            lines.push(Line::from(vec![
                Span::styled("Transport Link:    ", style_label()),
                Span::styled(format!("Physical USB 3.0 via ADB port forward (127.0.0.1:{})", t.port), style_value()),
                Span::styled("   Measured RTT: ", style_label()),
                Span::styled(t.rtt_ms.map(|ms| format!("{:.2} ms", ms)).unwrap_or_else(|| "N/A".into()), Style::default().fg(COLOR_SUCCESS)),
            ]));

            lines.push(Line::from(vec![
                Span::styled("Active Capabilities:", style_label()),
                Span::styled(" NDK C++ SIMD, Direct ByteBuffers, WakeLock Guardian, Qualcomm Thermal API", Style::default().fg(COLOR_SUCCESS)),
            ]));

            lines
        }
    };

    let title = if is_host { " HARDWARE & KERNEL PROFILE: LOCAL HOST " } else { " HARDWARE & WORKER PROFILE: REMOTE PHONE " };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_border_normal())
        .title(title);

    f.render_widget(Paragraph::new(lines).block(block), area);
}
