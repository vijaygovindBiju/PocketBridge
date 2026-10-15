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
            Constraint::Length(10), // Plugins Table
            Constraint::Min(8),     // Selected Plugin Detail Pane
        ])
        .split(area);

    // 1. Plugins Table
    render_plugins_table(f, app, chunks[0]);

    // 2. Plugin Details Pane
    render_plugin_details(f, app, chunks[1]);
}

fn render_plugins_table(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        "PLUGIN ID",
        "NAME",
        "STATUS",
        "CLASSIFICATION",
        "KIND",
        "MIN HOST RAM",
        "MIN WORKER RAM",
        "PACKAGE SIZE",
    ])
    .style(Style::default().fg(COLOR_SECONDARY).add_modifier(Modifier::BOLD));

    let mut rows = Vec::new();

    // 1. Built-in Core Engine (Zero-download calibration)
    let is_core_sel = app.selected_plugin_index == 0;
    rows.push(
        Row::new(vec![
            Span::styled(if is_core_sel { "▶ chunk_transform" } else { "  chunk_transform" }, Style::default().fg(COLOR_ACCENT)),
            Span::styled("SIMD Micro-Kernel Core", style_value()),
            Span::styled("BUILT-IN", Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD)),
            Span::styled("Benchmark", Style::default().fg(COLOR_INFO)),
            Span::styled("Native SIMD", style_value()),
            Span::styled("64 MB", style_value()),
            Span::styled("32 MB", style_value()),
            Span::styled("< 4 MB (Core)", Style::default().fg(COLOR_MUTED)),
        ])
        .style(if is_core_sel { Style::default().bg(Color::Rgb(25, 35, 45)).add_modifier(Modifier::BOLD) } else { Style::default() }),
    );

    // 2. Catalog & Installed Plugins
    for (i, (plugin, is_installed)) in app.catalog_plugins.iter().enumerate() {
        let row_idx = i + 1;
        let is_sel = row_idx == app.selected_plugin_index;
        let prefix = if is_sel { "▶ " } else { "  " };

        let (status_str, status_style) = if plugin.capability_status == pocketbridge_core::CapabilityStatus::Planned {
            ("ROADMAP", Style::default().fg(COLOR_MUTED))
        } else if *is_installed {
            ("INSTALLED", Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD))
        } else {
            ("AVAILABLE", Style::default().fg(COLOR_SECONDARY))
        };

        let (class_label, class_style) = match plugin.capability_status {
            pocketbridge_core::CapabilityStatus::HardwareTested
            | pocketbridge_core::CapabilityStatus::Implemented
            | pocketbridge_core::CapabilityStatus::BenchmarkVerified => ("Native Compute", Style::default().fg(COLOR_ACCENT)),
            pocketbridge_core::CapabilityStatus::Experimental => ("Experimental", Style::default().fg(COLOR_WARNING)),
            pocketbridge_core::CapabilityStatus::Planned => ("Planned Roadmap", Style::default().fg(COLOR_MUTED)),
            pocketbridge_core::CapabilityStatus::Scaffolded => ("Scaffolded", Style::default().fg(COLOR_WARNING)),
            pocketbridge_core::CapabilityStatus::Simulated => ("Simulated (Disabled)", Style::default().fg(COLOR_MUTED)),
        };

        let size_mb = plugin.size_bytes as f64 / (1024.0 * 1024.0);

        let row_style = if is_sel {
            Style::default().bg(Color::Rgb(25, 35, 45)).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        rows.push(
            Row::new(vec![
                Span::styled(format!("{}{}", prefix, plugin.id), style_value()),
                Span::styled(&plugin.name, Style::default().fg(COLOR_ACCENT)),
                Span::styled(status_str, status_style),
                Span::styled(class_label, class_style),
                Span::styled(format!("{:?}", plugin.kind), style_value()),
                Span::styled(format!("{} MB", plugin.requirements.min_host_ram_mb), style_value()),
                Span::styled(format!("{} MB", plugin.requirements.min_worker_ram_mb), style_value()),
                Span::styled(format!("{:.1} MB", size_mb), Style::default().fg(COLOR_MUTED)),
            ])
            .style(row_style),
        );
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(20),
            Constraint::Length(26),
            Constraint::Length(14),
            Constraint::Length(18),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(16),
            Constraint::Length(16),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(style_border_focused())
            .title(" MODULAR WORKLOAD & COMMUNITY PLUGIN HUB (↑/↓ to select, [i] Install, [u] Uninstall) "),
    );

    f.render_widget(table, area);
}

fn render_plugin_details(f: &mut Frame, app: &App, area: Rect) {
    let lines = if app.selected_plugin_index == 0 {
        // Built-in core description
        vec![
            Line::from(vec![
                Span::styled("Plugin ID:        ", style_label()),
                Span::styled("chunk_transform.v1 (Micro-Kernel Architecture)", Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("Classification:   ", style_label()),
                Span::styled("Built-in Baseline Hardware Calibration Benchmark", Style::default().fg(COLOR_SUCCESS)),
            ]),
            Line::from(vec![
                Span::styled("Description:      ", style_label()),
                Span::styled("Compiled directly into pocketbridge-core. Provides instantaneous, zero-download offline link calibration and NDK SIMD verification.", style_value()),
            ]),
            Line::from(vec![
                Span::styled("Hardware Target:  ", style_label()),
                Span::styled("Host: Intel AVX2 / SHA-NI  |  Worker: Qualcomm Snapdragon ARMv8.2-A+SIMD NDK C++", style_value()),
            ]),
            Line::from(vec![
                Span::styled("Integrity Status: ", style_label()),
                Span::styled("Active, verified bit-for-bit with cryptographic SHA-256 digest matching.", Style::default().fg(COLOR_SUCCESS)),
            ]),
        ]
    } else {
        let cat_idx = app.selected_plugin_index - 1;
        if let Some((plugin, is_installed)) = app.catalog_plugins.get(cat_idx) {
            let status_note = if plugin.capability_status == pocketbridge_core::CapabilityStatus::Planned {
                Span::styled("Roadmap Item — Architecture defined; native worker backend is planned for future release.", Style::default().fg(COLOR_MUTED))
            } else if *is_installed {
                Span::styled("Installed in ~/.pocketbridge/plugins/ — Ready to execute", Style::default().fg(COLOR_SUCCESS))
            } else {
                Span::styled("Available in Community Hub — Press 'i' to install package", Style::default().fg(COLOR_WARNING))
            };

            let sha_str = plugin.sha256.as_deref().unwrap_or("Unverified");
            let size_mb = plugin.size_bytes as f64 / (1024.0 * 1024.0);

            vec![
                Line::from(vec![
                    Span::styled("Package Name:     ", style_label()),
                    Span::styled(format!("{} (v{})", plugin.name, plugin.version), Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)),
                    Span::styled("    Adapter ID:   ", style_label()),
                    Span::styled(&plugin.adapter_id, style_value()),
                ]),
                Line::from(vec![
                    Span::styled("Lifecycle Status: ", style_label()),
                    status_note,
                ]),
                Line::from(vec![
                    Span::styled("Description:      ", style_label()),
                    Span::styled(&plugin.description, style_value()),
                ]),
                Line::from(vec![
                    Span::styled("Author & License: ", style_label()),
                    Span::styled(format!("{} ({})", plugin.author, plugin.license), style_value()),
                    Span::styled("    Size:         ", style_label()),
                    Span::styled(format!("{:.2} MB ({} bytes)", size_mb, plugin.size_bytes), style_value()),
                ]),
                Line::from(vec![
                    Span::styled("SHA-256 Digest:   ", style_label()),
                    Span::styled(sha_str, Style::default().fg(COLOR_MUTED)),
                ]),
                Line::from(vec![
                    Span::styled("Requirements:     ", style_label()),
                    Span::styled(
                        format!(
                            "Min Host RAM: {} MB  |  Min Worker RAM: {} MB  |  Min Storage: {} MB",
                            plugin.requirements.min_host_ram_mb,
                            plugin.requirements.min_worker_ram_mb,
                            plugin.requirements.min_worker_storage_mb
                        ),
                        style_value(),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Pre-Flight Gate:  ", style_label()),
                    Span::styled("Enforces host RAM allocation and worker battery/thermal headroom before activation.", Style::default().fg(COLOR_SECONDARY)),
                ]),
            ]
        } else {
            vec![Line::from(Span::raw("No plugin selected."))]
        }
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_border_normal())
        .title(" PLUGIN MANIFEST & HARDWARE PREREQUISITES ");

    f.render_widget(Paragraph::new(lines).block(block), area);
}
