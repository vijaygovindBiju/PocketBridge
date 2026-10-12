use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};
use super::app::App;
use pocketbridge_core::DeviceProfile;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Status & Link summary
            Constraint::Length(6), // Worker telemetry
            Constraint::Length(6), // Compute Task & Speedup Summary
            Constraint::Min(4),    // Activity log
        ])
        .split(area);

    // 1. Connection & Link Summary
    let (conn_status, conn_color) = if app.connected {
        ("CONNECTED", Color::Green)
    } else if app.connecting {
        ("CONNECTING...", Color::Yellow)
    } else {
        ("DISCONNECTED", Color::Red)
    };

    let device_text = app.device_name.as_deref().unwrap_or("None detected");
    let status_text = vec![
        Line::from(vec![
            Span::raw("Link State:      "),
            Span::styled(conn_status, Style::default().fg(conn_color).add_modifier(Modifier::BOLD)),
            Span::raw(format!("   (Port: 127.0.0.1:{})", app.port)),
        ]),
        Line::from(vec![
            Span::raw("ADB Device:      "),
            Span::styled(device_text, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("Status Note:     "),
            Span::styled(&app.status_message, Style::default().fg(Color::White)),
        ]),
    ];

    let status_block = Paragraph::new(status_text)
        .block(Block::default().borders(Borders::ALL).title(" Link & Connection "));
    f.render_widget(status_block, chunks[0]);

    // 2. Worker Telemetry & Status
    let worker_lines = if let Some(hb) = &app.last_heartbeat {
        vec![
            Line::from(vec![
                Span::raw("Uptime:          "),
                Span::styled(format!("{}s", hb.uptime_ms / 1000), Style::default().fg(Color::White)),
                Span::raw("   Battery: "),
                Span::styled(
                    format!("{}%", hb.battery_percent.unwrap_or(0)),
                    Style::default().fg(if hb.battery_percent.unwrap_or(0) > 20 { Color::Green } else { Color::Red }),
                ),
                Span::raw(format!(" (Charging: {:?})", hb.battery_charging.unwrap_or(false))),
                Span::raw("   Temp: "),
                Span::styled(
                    format!("{:.1}°C", hb.temperature_celsius.unwrap_or(0.0)),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw("   Thermal: "),
                Span::styled(
                    hb.thermal_status.as_deref().unwrap_or("normal"),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::raw("Worker RAM:      "),
                Span::styled(
                    format!(
                        "{} / {}",
                        DeviceProfile::format_ram(hb.memory_used_bytes.unwrap_or(0)),
                        DeviceProfile::format_ram(hb.memory_total_bytes.unwrap_or(0))
                    ),
                    Style::default().fg(Color::White),
                ),
            ]),
        ]
    } else if let Some(hello) = &app.worker_hello {
        vec![
            Line::from(vec![
                Span::raw("Worker Agent:    "),
                Span::styled(&hello.agent_name, Style::default().fg(Color::Green)),
                Span::raw(format!(" (v{})", hello.agent_version)),
            ]),
            Line::from(Span::raw("Awaiting first heartbeat telemetry...")),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                "Worker is not connected. Connect phone via USB-C and press 'c'.",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    };

    let worker_block = Paragraph::new(worker_lines)
        .block(Block::default().borders(Borders::ALL).title(" Worker Status & Telemetry "));
    f.render_widget(worker_block, chunks[1]);

    // 3. Compute Task Execution & Speedup Summary
    if app.job_running {
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(format!(" Task Computing ({} records) ", app.job_records)))
            .gauge_style(Style::default().fg(Color::LightGreen))
            .ratio(app.job_progress_pct as f64)
            .label(format!("{:.0}% — {}", app.job_progress_pct * 100.0, app.job_status_msg));
        f.render_widget(gauge, chunks[2]);
    } else if let Some(res) = &app.active_job_result {
        let speedup_span = match res.speedup {
            Some(s) if s > 1.0 => Span::styled(
                format!("{:.2}x FASTER remotely", s),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
            Some(s) => Span::styled(
                format!("{:.2}x (Local was faster due to transfer overhead)", s),
                Style::default().fg(Color::Yellow),
            ),
            None => Span::raw("-"),
        };

        let equiv_span = if res.outputs_match {
            Span::styled("MATCHED (Verified bit-for-bit)", Style::default().fg(Color::Green))
        } else {
            Span::styled("MISMATCH", Style::default().fg(Color::Red))
        };

        let task_lines = vec![
            Line::from(vec![
                Span::raw("Last Task:       "),
                Span::styled(&res.adapter_id, Style::default().fg(Color::Cyan)),
                Span::raw(format!(" ({} records)", res.records_processed)),
                Span::raw("   Equivalence: "),
                equiv_span,
            ]),
            Line::from(vec![
                Span::raw("Local Time:      "),
                Span::styled(format!("{:.2} ms", res.local_duration_ms.unwrap_or(0.0)), Style::default().fg(Color::White)),
                Span::raw("   Phone Time: "),
                Span::styled(format!("{:.2} ms", res.remote_compute_ms.unwrap_or(0.0)), Style::default().fg(Color::White)),
                Span::raw("   Total Remote: "),
                Span::styled(format!("{:.2} ms", res.remote_total_ms.unwrap_or(0.0)), Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::raw("Net Speedup:     "),
                speedup_span,
                Span::styled("  [Press 'j' to re-run]", Style::default().fg(Color::DarkGray)),
            ]),
        ];

        let task_block = Paragraph::new(task_lines)
            .block(Block::default().borders(Borders::ALL).title(" Compute Task & Speedup Summary "));
        f.render_widget(task_block, chunks[2]);
    } else {
        let placeholder = vec![
            Line::from(vec![
                Span::raw("No compute task executed yet. Press "),
                Span::styled("'j'", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(format!(" to offload a {} record numeric chunk to the phone.", app.job_records)),
            ]),
            Line::from(Span::styled(
                "PocketBridge will execute locally & remotely, verify SHA-256 equivalence, and report speedup.",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        let task_block = Paragraph::new(placeholder)
            .block(Block::default().borders(Borders::ALL).title(" Compute Task & Speedup Summary "));
        f.render_widget(task_block, chunks[2]);
    }

    // 4. Activity Log
    let log_items: Vec<ListItem> = app
        .logs
        .iter()
        .rev()
        .take(chunks[3].height as usize)
        .map(|log_entry| ListItem::new(Span::styled(log_entry, Style::default().fg(Color::Gray))))
        .collect();

    let logs_widget = List::new(log_items)
        .block(Block::default().borders(Borders::ALL).title(" Recent Events & Protocol Log "));
    f.render_widget(logs_widget, chunks[3]);
}
