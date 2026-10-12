use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table},
    Frame,
};
use super::app::App;
use pocketbridge_core::format_bytes;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Controls & config
            Constraint::Length(3), // Progress bar
            Constraint::Min(8),    // Results table
        ])
        .split(area);

    // 1. Controls
    let controls_text = vec![
        Line::from(vec![
            Span::raw("Runs per payload: "),
            Span::styled(format!("{}", app.runs), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" (use '+/-' to adjust)    Payloads: "),
            Span::styled("64B, 1KB, 1MB, 10MB, 100MB", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("Status: "),
            Span::styled(
                if app.benchmark_running {
                    format!("Running: {}", app.benchmark_stage)
                } else if !app.benchmark_results.is_empty() {
                    "Completed".into()
                } else {
                    "Ready to run. Press [Enter] to start.".into()
                },
                Style::default().fg(if app.benchmark_running { Color::Yellow } else { Color::Green }),
            ),
            if let Some(path) = &app.last_saved_report {
                Span::styled(format!("  [Saved: {}]", path), Style::default().fg(Color::DarkGray))
            } else {
                Span::raw("")
            },
        ]),
    ];

    let controls_block = Paragraph::new(controls_text)
        .block(Block::default().borders(Borders::ALL).title(" Benchmark Configuration "));
    f.render_widget(controls_block, chunks[0]);

    // 2. Progress Gauge
    let (completed, total) = app.benchmark_progress;
    let ratio = if total > 0 {
        (completed as f64 / total as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Progress "))
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(ratio)
        .label(format!("{}/{} ({:.0}%)", completed, total, ratio * 100.0));
    f.render_widget(gauge, chunks[1]);

    // 3. Results Table
    let header_cells = ["Type", "Size", "Runs", "Median (ms)", "P95 (ms)", "Min (ms)", "Max (ms)", "Throughput"]
        .into_iter()
        .map(|h| Cell::from(h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).bottom_margin(1);

    let rows: Vec<Row> = if app.benchmark_results.is_empty() {
        vec![Row::new(vec![Cell::from("No benchmark results yet. Press [Enter] to run.")])]
    } else {
        app.benchmark_results
            .iter()
            .map(|res| {
                let throughput_str = match res.stats.throughput_mbps {
                    Some(mbps) => format!("{:.2} Mbps", mbps),
                    None => "-".to_string(),
                };

                let cells = vec![
                    Cell::from(res.test_type.to_string()),
                    Cell::from(format_bytes(res.payload_size_bytes)),
                    Cell::from(format!("{}", res.runs)),
                    Cell::from(format!("{:.2}", res.stats.median_ms)),
                    Cell::from(format!("{:.2}", res.stats.p95_ms)),
                    Cell::from(format!("{:.2}", res.stats.min_ms)),
                    Cell::from(format!("{:.2}", res.stats.max_ms)),
                    Cell::from(throughput_str).style(Style::default().fg(Color::Green)),
                ];
                Row::new(cells)
            })
            .collect()
    };

    let widths = [
        Constraint::Length(16),
        Constraint::Length(12),
        Constraint::Length(8),
        Constraint::Length(14),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(16),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" Benchmark Measurements "));
    f.render_widget(table, chunks[2]);
}
