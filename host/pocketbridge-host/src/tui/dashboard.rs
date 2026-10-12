use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use super::app::App;
use pocketbridge_core::DeviceProfile;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // Status & Link summary
            Constraint::Length(7), // Worker telemetry
            Constraint::Min(5),    // Activity log
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
                Span::styled(format!("{} seconds", hb.uptime_ms / 1000), Style::default().fg(Color::White)),
                Span::raw("    Battery: "),
                Span::styled(
                    format!("{}%", hb.battery_percent.unwrap_or(0)),
                    Style::default().fg(if hb.battery_percent.unwrap_or(0) > 20 { Color::Green } else { Color::Red }),
                ),
                Span::raw(format!(" (Charging: {:?})", hb.battery_charging.unwrap_or(false))),
            ]),
            Line::from(vec![
                Span::raw("Temperature:     "),
                Span::styled(
                    format!("{:.1} °C", hb.temperature_celsius.unwrap_or(0.0)),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw("    Thermal Status: "),
                Span::styled(
                    hb.thermal_status.as_deref().unwrap_or("normal"),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::raw("Memory:          "),
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
                "Worker is not connected. Connect USB cable and press 'c'.",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    };

    let worker_block = Paragraph::new(worker_lines)
        .block(Block::default().borders(Borders::ALL).title(" Worker Status & Telemetry "));
    f.render_widget(worker_block, chunks[1]);

    // 3. Activity Log
    let log_items: Vec<ListItem> = app
        .logs
        .iter()
        .rev()
        .take(chunks[2].height as usize)
        .map(|log_entry| ListItem::new(Span::styled(log_entry, Style::default().fg(Color::Gray))))
        .collect();

    let logs_widget = List::new(log_items)
        .block(Block::default().borders(Borders::ALL).title(" Recent Events & Protocol Log "));
    f.render_widget(logs_widget, chunks[2]);
}
