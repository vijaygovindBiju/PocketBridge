use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Row, Table},
    Frame,
};
use super::app::{App, LogSeverity, TaskStatus};
use super::theme::*;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    // Adaptive vertical split
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Resources (Host + Worker side-by-side)
            Constraint::Length(4), // Transport Panel
            Constraint::Length(6), // Active Workload Table / Status
            Constraint::Min(4),    // Recent Events
        ])
        .split(area);

    // 1. Resources: Host and Worker side-by-side (or stacked if narrow)
    if area.width >= 70 {
        let res_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);
        render_host_resources(f, app, res_chunks[0]);
        render_worker_resources(f, app, res_chunks[1]);
    } else {
        let res_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);
        render_host_resources(f, app, res_chunks[0]);
        render_worker_resources(f, app, res_chunks[1]);
    }

    // 2. Transport Panel
    render_transport(f, app, chunks[1]);

    // 3. Active Workload Panel
    render_active_workload(f, app, chunks[2]);

    // 4. Recent Events
    render_recent_events(f, app, chunks[3]);
}

fn render_host_resources(f: &mut Frame, app: &App, area: Rect) {
    let host = &app.host;
    let ram_ratio = if host.ram_total_bytes > 0 {
        host.ram_used_bytes as f64 / host.ram_total_bytes as f64
    } else {
        0.0
    };

    let cpu_ratio = host.cpu_usage_pct / 100.0;

    let active_tasks_count = app.tasks.iter().filter(|t| t.status == TaskStatus::Running).count();

    let lines = vec![
        Line::from(vec![
            Span::styled("Memory:   ", style_label()),
            Span::styled(format!("{:<15}", format!("{} / {}", format_bytes(host.ram_used_bytes), format_bytes(host.ram_total_bytes))), style_value()),
            Span::raw(" "),
            render_mini_bar(ram_ratio, 16),
            Span::raw(" "),
            Span::styled(format!("(Avail: {})", format_bytes(host.ram_available_bytes)), Style::default().fg(COLOR_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("CPU:      ", style_label()),
            Span::styled(format!("{:>4.1}% ", host.cpu_usage_pct), style_value()),
            render_mini_bar(cpu_ratio, 16),
            Span::raw(" "),
            Span::styled(format!("({} cores)", host.cpu_cores), Style::default().fg(COLOR_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("Swap:     ", style_label()),
            Span::styled(format!("{} / {}", format_bytes(host.swap_used_bytes), format_bytes(host.swap_total_bytes)), style_value()),
            Span::styled("    Load: ", style_label()),
            Span::styled(format!("{:.2}, {:.2}, {:.2}", host.load_1m, host.load_5m, host.load_15m), style_value()),
        ]),
        Line::from(vec![
            Span::styled("Workloads:", style_label()),
            Span::styled(format!(" {} active", active_tasks_count), if active_tasks_count > 0 { Style::default().fg(COLOR_ACCENT) } else { style_value() }),
            Span::styled("        OS:   ", style_label()),
            Span::styled(format!("{} ({})", host.os, host.kernel), Style::default().fg(COLOR_SECONDARY)),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_border_normal())
        .title(" HOST RESOURCES (Arch Linux / x86_64) ");
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_worker_resources(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.transport;
    let worker_prof = app.worker_profile.as_ref();
    let dev_name = worker_prof
        .and_then(|p| p.device_model.clone())
        .or_else(|| t.adb_device.clone())
        .unwrap_or_else(|| "No Phone Attached".to_string());

    let lines = if t.is_connected {
        if let Some(hb) = &t.last_heartbeat {
            let used = hb.memory_used_bytes.unwrap_or(0);
            let total = hb.memory_total_bytes.unwrap_or(0);
            let mem_str = if total > 0 {
                format!("{} / {}", format_bytes(used), format_bytes(total))
            } else {
                "N/A".to_string()
            };

            let cpu_str = match hb.cpu_usage_percent {
                Some(pct) => format!("{:.1}%", pct),
                None => "N/A".to_string(),
            };

            let bat_str = match hb.battery_percent {
                Some(pct) => {
                    let chg = if hb.battery_charging.unwrap_or(false) { " · Charging" } else { " · Discharging" };
                    format!("{}%{}", pct, chg)
                }
                None => "N/A".to_string(),
            };

            let temp_str = match hb.temperature_celsius {
                Some(deg) => format!("{:.1} °C", deg),
                None => "N/A".to_string(),
            };

            let thermal_status = hb.thermal_status.as_deref().unwrap_or("normal");
            let age = t.age_string();
            let is_stale = t.is_stale();

            vec![
                Line::from(vec![
                    Span::styled("Memory:   ", style_label()),
                    Span::styled(format!("{:<15}", mem_str), style_value()),
                    Span::raw(" "),
                    Span::styled("[Direct/ActivityMgr]", Style::default().fg(COLOR_MUTED)),
                ]),
                Line::from(vec![
                    Span::styled("CPU:      ", style_label()),
                    Span::styled(format!("{:<8}", cpu_str), style_value()),
                    Span::styled("    Battery: ", style_label()),
                    Span::styled(bat_str, if hb.battery_percent.unwrap_or(100) > 20 { Style::default().fg(COLOR_SUCCESS) } else { Style::default().fg(COLOR_ERROR) }),
                ]),
                Line::from(vec![
                    Span::styled("Thermal:  ", style_label()),
                    Span::styled(format!("{} ({})", temp_str, thermal_status), Style::default().fg(COLOR_WARNING)),
                    Span::styled("   Uptime:  ", style_label()),
                    Span::styled(format!("{}s", hb.uptime_ms / 1000), style_value()),
                ]),
                Line::from(vec![
                    Span::styled("Telemetry:", style_label()),
                    Span::styled(format!(" {}", age), if is_stale { Style::default().fg(COLOR_WARNING) } else { Style::default().fg(COLOR_SECONDARY) }),
                    Span::styled("       Worker:  ", style_label()),
                    badge_ok("Online"),
                ]),
            ]
        } else {
            vec![
                Line::from(vec![
                    Span::styled("Device:   ", style_label()),
                    Span::styled(dev_name.clone(), style_value()),
                ]),
                Line::from(Span::styled("Connected. Awaiting first heartbeat telemetry frame...", Style::default().fg(COLOR_WARNING))),
            ]
        }
    } else if t.is_connecting {
        vec![
            Line::from(vec![
                Span::styled("Status:   ", style_label()),
                Span::styled("Handshaking worker over ADB tcp:9876...", Style::default().fg(COLOR_WARNING)),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("Status:   ", style_label()),
                Span::styled("Disconnected / Offline", Style::default().fg(COLOR_ERROR)),
            ]),
            Line::from(vec![
                Span::styled("Note:     ", style_label()),
                Span::styled("Connect phone via USB and press 'c' to handshake worker.", Style::default().fg(COLOR_MUTED)),
            ]),
        ]
    };

    let title = format!(" PHONE RESOURCES ({}) ", dev_name);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_border_normal())
        .title(title);
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_transport(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.transport;
    let (status_badge, border_style) = if t.is_connected {
        (badge_ok("CONNECTED"), style_border_normal())
    } else if t.is_connecting {
        (badge_warn("CONNECTING"), style_border_focused())
    } else {
        (badge_err("DISCONNECTED"), style_border_normal())
    };

    let rtt_str = match t.rtt_ms {
        Some(ms) => format_duration(ms),
        None => "N/A".to_string(),
    };

    let throughput_str = match t.measured_throughput_mbps {
        Some(mbps) => format!("{:.1} Mbps", mbps),
        None => "Unmeasured (Run benchmark)".to_string(),
    };

    let dev_id = t.adb_device.as_deref().unwrap_or("None detected");

    let lines = vec![
        Line::from(vec![
            Span::styled("Link: ", style_label()),
            Span::styled("ADB / USB 3.0", style_value()),
            Span::styled("   State: ", style_label()),
            status_badge,
            Span::styled("   Target: ", style_label()),
            Span::styled(format!("127.0.0.1:{}", t.port), Style::default().fg(COLOR_ACCENT)),
            Span::styled("   Device: ", style_label()),
            Span::styled(dev_id, style_value()),
        ]),
        Line::from(vec![
            Span::styled("Measured RTT: ", style_label()),
            Span::styled(rtt_str, Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD)),
            Span::styled("   Throughput: ", style_label()),
            Span::styled(throughput_str, style_value()),
            Span::styled("   Heartbeat: ", style_label()),
            Span::styled(t.age_string(), if t.is_stale() { Style::default().fg(COLOR_WARNING) } else { style_value() }),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" TRANSPORT & PHYSICAL LINK ");
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_active_workload(f: &mut Frame, app: &App, area: Rect) {
    if let Some(task_id) = &app.active_task_id {
        // Render running progress bar
        let gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(style_border_focused())
                    .title(format!(" ACTIVE WORKLOAD (Job {}) ", &task_id[..task_id.len().min(8)])),
            )
            .gauge_style(Style::default().fg(COLOR_ACCENT))
            .ratio(app.active_task_progress as f64)
            .label(format!("{:.0}% — {}", app.active_task_progress * 100.0, app.active_task_status_msg));
        f.render_widget(gauge, area);
    } else {
        // Render compact recent/last workload table
        let header = Row::new(vec!["TASK ID", "WORKLOAD", "CLASSIFICATION", "MODE", "STATE", "DURATION", "SPEEDUP"])
            .style(Style::default().fg(COLOR_SECONDARY).add_modifier(Modifier::BOLD));

        let rows: Vec<Row> = app
            .tasks
            .iter()
            .rev()
            .take(2)
            .map(|t| {
                let id_short = if t.id.len() >= 8 { &t.id[..8] } else { &t.id };
                let (state_span, state_style) = match t.status {
                    TaskStatus::Succeeded => ("Succeeded", Style::default().fg(COLOR_SUCCESS)),
                    TaskStatus::Failed => ("Failed", Style::default().fg(COLOR_ERROR)),
                    TaskStatus::Running => ("Running", Style::default().fg(COLOR_ACCENT)),
                    TaskStatus::Queued => ("Queued", Style::default().fg(COLOR_WARNING)),
                    TaskStatus::Cancelled => ("Cancelled", Style::default().fg(COLOR_MUTED)),
                };

                let speedup_str = match t.speedup {
                    Some(s) => format!("{:.2}x", s),
                    None => "-".to_string(),
                };

                let dur_str = match t.duration_ms {
                    Some(ms) => format_duration(ms),
                    None => "-".to_string(),
                };

                Row::new(vec![
                    Span::styled(id_short, style_value()),
                    Span::styled(&t.name, Style::default().fg(COLOR_ACCENT)),
                    Span::styled(t.classification.label(), Style::default().fg(COLOR_MUTED)),
                    Span::styled(t.mode.to_string(), style_value()),
                    Span::styled(state_span, state_style),
                    Span::styled(dur_str, style_value()),
                    Span::styled(speedup_str, Style::default().fg(COLOR_SUCCESS)),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(18),
                Constraint::Length(18),
                Constraint::Length(14),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(style_border_normal())
                .title(" ACTIVE / RECENT WORKLOAD "),
        );

        f.render_widget(table, area);
    }
}

fn render_recent_events(f: &mut Frame, app: &App, area: Rect) {
    let recent_logs: Vec<Line> = app
        .logs
        .iter()
        .rev()
        .take(5)
        .map(|entry| {
            let (sev_badge, sev_color) = match entry.severity {
                LogSeverity::Info => ("[INFO]", COLOR_INFO),
                LogSeverity::Warn => ("[WARN]", COLOR_WARNING),
                LogSeverity::Error => ("[ERR ]", COLOR_ERROR),
                LogSeverity::Debug => ("[DBG ]", COLOR_MUTED),
            };

            Line::from(vec![
                Span::styled(format!("{} ", entry.timestamp), Style::default().fg(COLOR_MUTED)),
                Span::styled(format!("{:<6} ", sev_badge), Style::default().fg(sev_color)),
                Span::styled(format!("[{:<9}] ", entry.component), Style::default().fg(COLOR_SECONDARY)),
                Span::styled(&entry.message, style_value()),
            ])
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_border_normal())
        .title(" RECENT SYSTEM EVENTS ");
    f.render_widget(Paragraph::new(recent_logs).block(block), area);
}
