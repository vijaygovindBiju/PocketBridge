use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use super::app::App;
use pocketbridge_core::ExecutionTarget;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Top: Config & Constraints
            Constraint::Min(12),    // Bottom: Cost Model Estimates & Decision Card
        ])
        .split(area);

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    // 1. Workload & Model Config (Top Left)
    let input_bytes = (app.job_records as usize) * 8;
    let output_bytes = (app.job_records as usize) * 8;
    let input_mb = input_bytes as f64 / (1024.0 * 1024.0);
    let output_mb = output_bytes as f64 / (1024.0 * 1024.0);

    let config_lines = vec![
        Line::from(vec![
            Span::raw("Workload Records:   "),
            Span::styled(format!("{}", app.job_records), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(format!(" (Payload: {:.2} MB in / {:.2} MB out)", input_mb, output_mb)),
        ]),
        Line::from(vec![
            Span::raw("Compute Rounds:     "),
            Span::styled(format!("{}", app.job_rounds), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("Simulated Host Load:"),
            Span::styled(
                format!("{:.0}% available CPU capacity (factor: {:.2})", app.scheduler_host_load * 100.0, app.scheduler_host_load),
                if app.scheduler_host_load < 0.3 { Style::default().fg(Color::Magenta) } else { Style::default().fg(Color::Cyan) },
            ),
        ]),
        Line::from(vec![
            Span::raw("Required Margin:    "),
            Span::styled(format!("{:.2}x minimum speedup threshold", app.scheduler_margin), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Controls: [p] Re-evaluate | [h/H] Adjust Host Load | [+/-] Records", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let config_block = Paragraph::new(config_lines)
        .block(Block::default().borders(Borders::ALL).title(" Workload & Scheduler Config "));
    f.render_widget(config_block, top_chunks[0]);

    // 2. Hardware Constraints (Top Right)
    let constraint_lines = if let Some(decision) = &app.scheduler_decision {
        let c = &decision.constraints;
        vec![
            Line::from(vec![
                Span::raw("Battery Constraint: "),
                Span::styled(
                    if c.battery_ok { "PASSED" } else { "FAILED" },
                    Style::default().fg(if c.battery_ok { Color::Green } else { Color::Red }).add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" (Level: {}%, Charging: {})", c.battery_percent.unwrap_or(0), c.battery_charging.unwrap_or(false))),
            ]),
            Line::from(vec![
                Span::raw("Thermal Constraint: "),
                Span::styled(
                    if c.thermal_ok { "PASSED" } else { "FAILED" },
                    Style::default().fg(if c.thermal_ok { Color::Green } else { Color::Red }).add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" (Status: {})", c.thermal_status.as_deref().unwrap_or("normal"))),
            ]),
            Line::from(vec![
                Span::raw("Memory Headroom:    "),
                Span::styled(
                    if c.memory_ok { "PASSED" } else { "FAILED" },
                    Style::default().fg(if c.memory_ok { Color::Green } else { Color::Red }).add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" (Req: {:.1} MB, Headroom safe)", c.memory_required_bytes as f64 / (1024.0 * 1024.0))),
            ]),
            Line::from(vec![
                Span::raw("Link Latency:       "),
                Span::styled(
                    if c.link_ok { "PASSED" } else { "FAILED" },
                    Style::default().fg(if c.link_ok { Color::Green } else { Color::Red }).add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" (RTT: {:.1} ms, Max: 250ms)", c.link_rtt_ms.unwrap_or(0.0))),
            ]),
            Line::from(vec![
                Span::raw("All Safety Checks:  "),
                Span::styled(
                    if c.all_passed() { "ALL CONSTRAINTS SATISFIED" } else { "SAFETY VIOLATION PREVENTS OFFLOAD" },
                    Style::default().fg(if c.all_passed() { Color::Green } else { Color::Red }).add_modifier(Modifier::BOLD),
                ),
            ]),
        ]
    } else {
        vec![
            Line::from(Span::styled("No scheduler evaluation yet. Press 'p' to evaluate.", Style::default().fg(Color::DarkGray))),
        ]
    };

    let constraint_block = Paragraph::new(constraint_lines)
        .block(Block::default().borders(Borders::ALL).title(" Hardware Safety Constraints "));
    f.render_widget(constraint_block, top_chunks[1]);

    // 3. Analytical Cost Model & Decision Card (Bottom)
    let decision_lines = if let Some(decision) = &app.scheduler_decision {
        let est = &decision.estimates;
        let (badge_text, badge_color) = match decision.target {
            ExecutionTarget::RemoteWorker => (
                "[RECOMMENDATION: OFFLOAD TO ANDROID WORKER]",
                Color::Green,
            ),
            ExecutionTarget::LocalHost => (
                "[RECOMMENDATION: EXECUTE LOCALLY ON HOST]",
                Color::Cyan,
            ),
            ExecutionTarget::HybridSplit { .. } => (
                "[RECOMMENDATION: HYBRID CO-PROCESSING SPLIT]",
                Color::Yellow,
            ),
        };

        vec![
            Line::from(vec![
                Span::raw("Scheduler Status:    "),
                Span::styled(badge_text, Style::default().fg(badge_color).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("Decision Rationale:  "),
                Span::styled(&decision.decision_reason, Style::default().fg(Color::White)),
            ]),
            Line::from(Span::raw("--------------------------------------------------------------------------")),
            Line::from(vec![
                Span::raw("Estimated Local Time:      "),
                Span::styled(format!("{:.2} ms", est.local_compute_ms), Style::default().fg(Color::White)),
                Span::raw("  (Multi-core Intel Core i5 reference)"),
            ]),
            Line::from(vec![
                Span::raw("Estimated Link Transfer:   "),
                Span::styled(
                    format!("{:.2} ms", est.transfer_in_ms + est.transfer_out_ms),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(format!("  (In: {:.2}ms, Out: {:.2}ms over USB-C ADB)", est.transfer_in_ms, est.transfer_out_ms)),
            ]),
            Line::from(vec![
                Span::raw("Estimated Remote Compute:  "),
                Span::styled(format!("{:.2} ms", est.remote_compute_ms), Style::default().fg(Color::Cyan)),
                Span::raw("  (8x Qualcomm Oryon CPU cores)"),
            ]),
            Line::from(vec![
                Span::raw("Estimated Remote Total:    "),
                Span::styled(format!("{:.2} ms", est.remote_total_ms), Style::default().fg(Color::Magenta)),
                Span::raw("  (Link Transfer + Remote Compute)"),
            ]),
            Line::from(vec![
                Span::raw("Predicted Speedup Ratio:   "),
                Span::styled(
                    format!("{:.2}x", est.predicted_speedup),
                    Style::default().fg(if est.predicted_speedup >= decision.speedup_margin { Color::Green } else { Color::Yellow }).add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("  (Required Policy Threshold: {:.2}x)", decision.speedup_margin)),
            ]),
        ]
    } else {
        vec![
            Line::from(Span::styled("Press 'p' to run Explainable Cost Model and Constraint Evaluation.", Style::default().fg(Color::Yellow))),
        ]
    };

    let decision_block = Paragraph::new(decision_lines)
        .block(Block::default().borders(Borders::ALL).title(" Explainable Cost Model & Decision Recommendation "));
    f.render_widget(decision_block, chunks[1]);
}
