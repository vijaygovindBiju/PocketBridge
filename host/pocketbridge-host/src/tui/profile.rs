use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use super::app::App;
use pocketbridge_core::DeviceProfile;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: Host Profile
    let host_lines = if let Some(p) = &app.host_profile {
        format_profile_lines(p)
    } else {
        vec![Line::from(Span::styled("Loading host profile...", Style::default().fg(Color::DarkGray)))]
    };

    let host_block = Paragraph::new(host_lines)
        .block(Block::default().borders(Borders::ALL).title(" Host System (Arch Linux) "));
    f.render_widget(host_block, chunks[0]);

    // Right: Worker Profile
    let worker_lines = if let Some(p) = &app.worker_profile {
        format_profile_lines(p)
    } else {
        vec![Line::from(Span::styled(
            "Worker profile unavailable. Connect phone and press 'r' to refresh.",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let worker_block = Paragraph::new(worker_lines)
        .block(Block::default().borders(Borders::ALL).title(" Worker Device (Android / Samsung Galaxy) "));
    f.render_widget(worker_block, chunks[1]);
}

fn format_profile_lines(p: &DeviceProfile) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    lines.push(Line::from(vec![
        Span::raw("Operating System:   "),
        Span::styled(format!("{} ({})", p.os, p.os_version), Style::default().fg(Color::Green)),
    ]));

    lines.push(Line::from(vec![
        Span::raw("Architecture:       "),
        Span::styled(p.architecture.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]));

    lines.push(Line::from(vec![
        Span::raw("CPU Model:          "),
        Span::styled(p.cpu_model.clone(), Style::default().fg(Color::White)),
    ]));

    lines.push(Line::from(vec![
        Span::raw("CPU Cores:          "),
        Span::styled(format!("{}", p.cpu_cores), Style::default().fg(Color::Yellow)),
    ]));

    if p.ram_total_bytes > 0 {
        lines.push(Line::from(vec![
            Span::raw("Total RAM:          "),
            Span::styled(DeviceProfile::format_ram(p.ram_total_bytes), Style::default().fg(Color::White)),
        ]));
    }

    if let Some(avail) = p.ram_available_bytes {
        lines.push(Line::from(vec![
            Span::raw("Available RAM:      "),
            Span::styled(DeviceProfile::format_ram(avail), Style::default().fg(Color::White)),
        ]));
    }

    if let Some(gpu) = &p.gpu {
        lines.push(Line::from(vec![
            Span::raw("GPU / Graphics:     "),
            Span::styled(gpu.clone(), Style::default().fg(Color::Yellow)),
        ]));
    }

    if let Some(dev) = &p.device_model {
        lines.push(Line::from(vec![
            Span::raw("Device Model:       "),
            Span::styled(dev.clone(), Style::default().fg(Color::Cyan)),
        ]));
    }

    if let Some(soc) = &p.soc {
        lines.push(Line::from(vec![
            Span::raw("SoC / Chipset:      "),
            Span::styled(soc.clone(), Style::default().fg(Color::White)),
        ]));
    }

    if let Some(api) = p.android_api_level {
        lines.push(Line::from(vec![
            Span::raw("Android API Level:  "),
            Span::styled(format!("{api}"), Style::default().fg(Color::Yellow)),
        ]));
    }

    if let Some(bat) = p.battery_percent {
        lines.push(Line::from(vec![
            Span::raw("Battery:            "),
            Span::styled(
                format!("{}% (Charging: {:?})", bat, p.battery_charging.unwrap_or(false)),
                Style::default().fg(if bat > 20 { Color::Green } else { Color::Red }),
            ),
        ]));
    }

    if let Some(therm) = &p.thermal_status {
        lines.push(Line::from(vec![
            Span::raw("Thermal Status:     "),
            Span::styled(therm.clone(), Style::default().fg(Color::Cyan)),
        ]));
    }

    lines
}
