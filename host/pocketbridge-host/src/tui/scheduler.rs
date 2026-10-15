use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use super::app::App;
use super::theme::*;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12), // Decision Card & Constraint Checklist
            Constraint::Length(8),  // Cost Model & Transfer Estimates
            Constraint::Min(4),     // Scheduler Controls & Experiment Disclaimers
        ])
        .split(area);

    // 1. Decision Card
    render_decision_card(f, app, chunks[0]);

    // 2. Cost Model Equations & Estimates
    render_cost_model(f, app, chunks[1]);

    // 3. Controls & Notes
    render_controls_and_notes(f, app, chunks[2]);
}

fn render_decision_card(f: &mut Frame, app: &App, area: Rect) {
    let dec_opt = app.scheduler_decision.as_ref();

    let lines = if let Some(d) = dec_opt {
        let (decision_badge, reason_color) = if d.approved_for_offload {
            (badge_ok("OFFLOAD RECOMMENDED"), COLOR_SUCCESS)
        } else {
            (badge_warn("EXECUTE LOCALLY ON HOST"), COLOR_WARNING)
        };

        let mem_check = if d.constraints.memory_ok {
            "[✓] Passed (Worker has sufficient headroom)"
        } else {
            "[✗] Failed or unverified memory headroom"
        };
        let bat_check = if d.constraints.battery_ok {
            "[✓] Passed (Battery > 15% or USB charging)"
        } else {
            "[✗] Failed or unverified battery gate"
        };
        let thermal_check = if d.constraints.thermal_ok {
            "[✓] Passed (Thermals: Normal / zero throttling)"
        } else {
            "[!] Thermal warning or throttle detected"
        };
        let rtt_check = if d.constraints.link_ok {
            "[✓] Passed (USB RTT < 50 ms latency threshold)"
        } else {
            "[✗] Failed (Link RTT > 50 ms or disconnected)"
        };

        vec![
            Line::from(vec![
                Span::styled("EVALUATED TASK:     ", style_label()),
                Span::styled(format!("chunk_transform ({} records, {} rounds)", app.scheduler_records, app.scheduler_rounds), style_value()),
            ]),
            Line::from(vec![
                Span::styled("SCHEDULER DECISION: ", style_label()),
                decision_badge,
                Span::raw("   "),
                Span::styled(format!("Predicted Speedup: {:.2}x", d.estimates.predicted_speedup), Style::default().fg(reason_color).add_modifier(Modifier::BOLD)),
                Span::styled(format!(" (Required margin: {:.2}x)", app.scheduler_margin), Style::default().fg(COLOR_MUTED)),
            ]),
            Line::from(vec![
                Span::styled("PRIMARY RATIONALE:  ", style_label()),
                Span::styled(&d.decision_reason, Style::default().fg(COLOR_FG)),
            ]),
            Line::from(Span::raw("")),
            Line::from(vec![
                Span::styled("HARDWARE SAFETY CONSTRAINTS CHECKLIST:", style_label()),
            ]),
            Line::from(vec![
                Span::styled("  * Memory Constraint:   ", style_label()),
                Span::styled(mem_check, Style::default().fg(COLOR_SUCCESS)),
            ]),
            Line::from(vec![
                Span::styled("  * Battery Power Gate:  ", style_label()),
                Span::styled(bat_check, Style::default().fg(COLOR_SUCCESS)),
            ]),
            Line::from(vec![
                Span::styled("  * Thermal Headroom:    ", style_label()),
                Span::styled(thermal_check, Style::default().fg(COLOR_SUCCESS)),
            ]),
            Line::from(vec![
                Span::styled("  * Transport Latency:   ", style_label()),
                Span::styled(rtt_check, Style::default().fg(COLOR_SUCCESS)),
            ]),
        ]
    } else {
        vec![
            Line::from(Span::styled("Scheduler has not evaluated a task plan yet.", Style::default().fg(COLOR_MUTED))),
            Line::from(Span::raw("Press 'p' or adjust workload parameters with '+/-' to trigger explainable evaluation.")),
        ]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_border_focused())
        .title(" EXPLAINABLE SCHEDULING DECISION & SAFETY GATES ");

    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_cost_model(f: &mut Frame, app: &App, area: Rect) {
    let dec_opt = app.scheduler_decision.as_ref();

    let lines = if let Some(d) = dec_opt {
        vec![
            Line::from(vec![
                Span::styled("ANALYTICAL COST MODEL: ", style_label()),
                Span::styled("Speedup = T_local / (T_transfer_in + T_remote_compute + T_transfer_out)", Style::default().fg(COLOR_ACCENT)),
            ]),
            Line::from(vec![
                Span::styled("  * Predicted Local Compute:   ", style_label()),
                Span::styled(format_duration(d.estimates.local_compute_ms), style_value()),
                Span::styled(" (Host capacity factor: ", Style::default().fg(COLOR_MUTED)),
                Span::styled(format!("{:.0}%", app.scheduler_host_load * 100.0), Style::default().fg(COLOR_ACCENT)),
                Span::styled(")", Style::default().fg(COLOR_MUTED)),
            ]),
            Line::from(vec![
                Span::styled("  * Predicted Remote Compute:  ", style_label()),
                Span::styled(format_duration(d.estimates.remote_compute_ms), style_value()),
                Span::styled(" (ARM NDK C++ SIMD engine)", Style::default().fg(COLOR_MUTED)),
            ]),
            Line::from(vec![
                Span::styled("  * Predicted USB Transfer:    ", style_label()),
                Span::styled(format_duration(d.estimates.transfer_in_ms + d.estimates.transfer_out_ms), style_value()),
                Span::styled(" (Link RTT + Payload framing)", Style::default().fg(COLOR_MUTED)),
            ]),
            Line::from(vec![
                Span::styled("  * Total Remote Overhead:     ", style_label()),
                Span::styled(format_duration(d.estimates.remote_total_ms), Style::default().fg(COLOR_WARNING)),
            ]),
        ]
    } else {
        vec![
            Line::from(Span::styled("Awaiting scheduler calculation...", Style::default().fg(COLOR_MUTED))),
        ]
    };


    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_border_normal())
        .title(" COST MODEL LATENCY BREAKDOWN ");

    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_controls_and_notes(f: &mut Frame, app: &App, area: Rect) {
    let lines = vec![
        Line::from(vec![
            Span::styled("SIMULATION CONTROLS: ", style_label()),
            Span::styled("[h] ", Style::default().fg(COLOR_ACCENT)),
            Span::styled(format!("Toggle Host Load ({:.0}%)   ", app.scheduler_host_load * 100.0), style_value()),
            Span::styled("[+/-] ", Style::default().fg(COLOR_ACCENT)),
            Span::styled(format!("Workload Records ({})   ", app.scheduler_records), style_value()),
            Span::styled("[p] ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Re-evaluate Plan", style_value()),
        ]),
        Line::from(Span::styled(
            "Note: This is an analytical resource-allocation decision based on hardware capabilities and measured link latency.",
            Style::default().fg(COLOR_MUTED),
        )),
        Line::from(Span::styled(
            "It establishes safety bounds; it does not claim mathematically global optimality across arbitrary non-deterministic tasks.",
            Style::default().fg(COLOR_MUTED),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_border_normal())
        .title(" POLICY PARAMETERS & SCIENTIFIC INTEGRITY ");

    f.render_widget(Paragraph::new(lines).block(block), area);
}
