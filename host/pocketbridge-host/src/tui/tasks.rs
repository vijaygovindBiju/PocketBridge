use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};
use super::app::{App, TaskStatus, WorkloadClassification};
use super::theme::*;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Tasks Table
            Constraint::Min(8),    // Selected Task Detail Pane
        ])
        .split(area);

    // 1. Tasks Table
    render_tasks_table(f, app, chunks[0]);

    // 2. Selected Task Detail Pane
    render_task_detail_pane(f, app, chunks[1]);
}

fn render_tasks_table(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        "TASK ID",
        "WORKLOAD",
        "CLASSIFICATION",
        "MODE",
        "ASSIGNED NODE",
        "STATE",
        "DURATION",
        "SPEEDUP",
        "SHA-256 VERIFIED",
    ])
    .style(Style::default().fg(COLOR_SECONDARY).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = if app.tasks.is_empty() {
        vec![Row::new(vec![
            Span::styled("No tasks executed yet.", Style::default().fg(COLOR_MUTED)),
            Span::raw(""),
            Span::raw(""),
            Span::raw(""),
            Span::raw(""),
            Span::raw(""),
            Span::raw(""),
            Span::raw(""),
            Span::raw(""),
        ])]
    } else {
        app.tasks
            .iter()
            .enumerate()
            .map(|(idx, t)| {
                let is_selected = idx == app.selected_task_index;
                let id_short = if t.id.len() >= 8 { &t.id[..8] } else { &t.id };

                let prefix = if is_selected { "▶ " } else { "  " };

                let (class_style, class_label) = match t.classification {
                    WorkloadClassification::Synthetic => (Style::default().fg(COLOR_WARNING), "Synthetic"),
                    WorkloadClassification::Benchmark => (Style::default().fg(COLOR_INFO), "Benchmark"),
                    WorkloadClassification::Simulated => (Style::default().fg(COLOR_MUTED), "Simulated"),
                    WorkloadClassification::RealInference => (Style::default().fg(COLOR_SUCCESS), "Real Inference"),
                    WorkloadClassification::RealComputation => (Style::default().fg(COLOR_ACCENT), "Real Compute"),
                    WorkloadClassification::Unsupported => (Style::default().fg(COLOR_ERROR), "Unsupported"),
                };

                let (state_str, state_style) = match t.status {
                    TaskStatus::Succeeded => ("Succeeded", Style::default().fg(COLOR_SUCCESS)),
                    TaskStatus::Failed => ("Failed", Style::default().fg(COLOR_ERROR)),
                    TaskStatus::Running => ("Running", Style::default().fg(COLOR_ACCENT)),
                    TaskStatus::Queued => ("Queued", Style::default().fg(COLOR_WARNING)),
                    TaskStatus::Cancelled => ("Cancelled", Style::default().fg(COLOR_MUTED)),
                };

                let speedup_str = match t.speedup {
                    Some(s) if s > 1.0 => format!("{:.2}x", s),
                    Some(s) => format!("{:.2}x", s),
                    None => "-".to_string(),
                };

                let dur_str = match t.duration_ms {
                    Some(ms) => format_duration(ms),
                    None => "-".to_string(),
                };

                let sha_str = match &t.sha256 {
                    Some(digest) if digest.len() >= 8 => format!("{}...", &digest[..8]),
                    Some(digest) => digest.clone(),
                    None => "-".to_string(),
                };

                let row_style = if is_selected {
                    Style::default().bg(Color::Rgb(25, 35, 45)).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Span::styled(format!("{}{}", prefix, id_short), style_value()),
                    Span::styled(&t.name, Style::default().fg(COLOR_ACCENT)),
                    Span::styled(class_label, class_style),
                    Span::styled(t.mode.to_string(), style_value()),
                    Span::styled(&t.target_device, style_value()),
                    Span::styled(state_str, state_style),
                    Span::styled(dur_str, style_value()),
                    Span::styled(speedup_str, Style::default().fg(COLOR_SUCCESS)),
                    Span::styled(sha_str, Style::default().fg(COLOR_SECONDARY)),
                ])
                .style(row_style)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(20),
            Constraint::Length(16),
            Constraint::Length(18),
            Constraint::Length(18),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(14),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(style_border_focused())
            .title(" TASK EXECUTION QUEUE & HISTORY (Use ↑/↓ to navigate, [j] Run Fast Job) "),
    );

    f.render_widget(table, area);
}

fn render_task_detail_pane(f: &mut Frame, app: &App, area: Rect) {
    let lines = if let Some(t) = app.tasks.get(app.selected_task_index) {
        let (class_note, class_color) = match t.classification {
            WorkloadClassification::Synthetic => (
                "Synthetic stress kernel: High-throughput memory-bandwidth & SIMD reduction. (NOT true LLM autoregression)",
                COLOR_WARNING,
            ),
            WorkloadClassification::Benchmark => (
                "Link / Core calibration benchmark: Generates reproducible deterministic test matrices.",
                COLOR_INFO,
            ),
            WorkloadClassification::Simulated => (
                "Simulated pipeline: Measures resource allocation and offload boundary lifecycle without live dependencies.",
                COLOR_MUTED,
            ),
            WorkloadClassification::RealInference => (
                "Real model inference: Bit-level autoregressive token generation across devices.",
                COLOR_SUCCESS,
            ),
            WorkloadClassification::RealComputation => (
                "Real computation: Bit-exact image or table data transformation.",
                COLOR_ACCENT,
            ),
            WorkloadClassification::Unsupported => (
                "Unsupported capability: Staged for future native NDK engine implementation.",
                COLOR_ERROR,
            ),
        };

        let mut out = vec![
            Line::from(vec![
                Span::styled("Task Identifier:     ", style_label()),
                Span::styled(&t.id, Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled("    Adapter:          ", style_label()),
                Span::styled(&t.adapter_id, style_value()),
            ]),
            Line::from(vec![
                Span::styled("Classification:      ", style_label()),
                Span::styled(t.classification.label(), Style::default().fg(class_color).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  — {}", class_note), Style::default().fg(COLOR_SECONDARY)),
            ]),
            Line::from(vec![
                Span::styled("Execution Mode:      ", style_label()),
                Span::styled(t.mode.to_string(), style_value()),
                Span::styled("    Target Device:    ", style_label()),
                Span::styled(&t.target_device, style_value()),
                Span::styled("    Records Processed: ", style_label()),
                Span::styled(format!("{}", t.records), style_value()),
            ]),
            Line::from(vec![
                Span::styled("Parameters:          ", style_label()),
                Span::styled(&t.parameters_summary, style_value()),
            ]),
        ];

        if let Some(tb) = &t.timings {
            out.push(Line::from(vec![
                Span::styled("Timing Breakdown:    ", style_label()),
                Span::styled(format!("Upload: {}", tb.upload_ms.map(format_duration).unwrap_or_else(|| "N/A".into())), style_value()),
                Span::styled(" | ", Style::default().fg(COLOR_MUTED)),
                Span::styled(format!("Remote Compute: {}", tb.compute_ms.map(format_duration).unwrap_or_else(|| "N/A".into())), Style::default().fg(COLOR_ACCENT)),
                Span::styled(" | ", Style::default().fg(COLOR_MUTED)),
                Span::styled(format!("Download: {}", tb.download_ms.map(format_duration).unwrap_or_else(|| "N/A".into())), style_value()),
                Span::styled(" | ", Style::default().fg(COLOR_MUTED)),
                Span::styled(format!("Total Wall-Clock: {}", tb.total_ms.map(format_duration).unwrap_or_else(|| "N/A".into())), Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD)),
            ]));
        }

        if let Some(sha) = &t.sha256 {
            out.push(Line::from(vec![
                Span::styled("Result Integrity:    ", style_label()),
                Span::styled("Bit-for-bit verified against host reference ", Style::default().fg(COLOR_SUCCESS)),
                Span::styled(format!("(SHA-256: {})", sha), Style::default().fg(COLOR_MUTED)),
            ]));
        }

        if let Some(err) = &t.error {
            out.push(Line::from(vec![
                Span::styled("Error Details:       ", style_label()),
                Span::styled(err, Style::default().fg(COLOR_ERROR)),
            ]));
        }

        out
    } else {
        vec![
            Line::from(Span::styled("No task selected.", Style::default().fg(COLOR_MUTED))),
            Line::from(Span::raw("Execute a compute job with 'j' to inspect full timing and verification metrics.")),
        ]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_border_normal())
        .title(" SELECTED TASK INSPECTOR & TIMING BREAKDOWN ");

    f.render_widget(Paragraph::new(lines).block(block), area);
}
