#![allow(dead_code)]
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Row, Table},
    Frame,
};
use super::app::App;
use super::theme::*;

pub struct BenchmarkTarget {
    pub name: &'static str,
    pub adapter_id: &'static str,
    pub classification: &'static str,
    pub description: &'static str,
    pub is_supported: bool,
    pub default_records: u32,
}

pub fn supported_benchmarks() -> &'static [BenchmarkTarget] {
    &[
        BenchmarkTarget {
            name: "USB Link Latency & Echo Sweep",
            adapter_id: "link_echo.v1",
            classification: "Transport Benchmark",
            description: "Measures 64B, 1KB pings and 1MB, 10MB, 100MB throughput round-trips over physical USB link.",
            is_supported: true,
            default_records: 20,
        },
        BenchmarkTarget {
            name: "SIMD Core Transform Calibration",
            adapter_id: "chunk_transform.v1",
            classification: "Synthetic Benchmark",
            description: "PRNG bit-diffusion and AVX2 / ARM NDK SIMD transform with bit-exact SHA-256 validation.",
            is_supported: true,
            default_records: 50_000,
        },
        BenchmarkTarget {
            name: "Image Batch Parallel Filter",
            adapter_id: "image_batch.v1",
            classification: "Real Computation",
            description: "Multi-threaded RGBA inversion, downscaling and box blur across laptop and phone.",
            is_supported: true,
            default_records: 2_000,
        },
        BenchmarkTarget {
            name: "Data Analytics Table Reduction",
            adapter_id: "data_analytics.v1",
            classification: "Real Computation",
            description: "Parallel CSV numerical aggregation, floating-point sum, min/max, and outlier filtering.",
            is_supported: true,
            default_records: 100_000,
        },
        BenchmarkTarget {
            name: "GGUF Model Layer Partition Analysis",
            adapter_id: "model_plan.v1",
            classification: "Model Inspection",
            description: "Gemma 2 27B binary inspection: parses 508 tensors, calculates layer splits for dual-device memory.",
            is_supported: true,
            default_records: 46,
        },
        BenchmarkTarget {
            name: "Distributed LLM Token Generation",
            adapter_id: "llm_pipeline.v1",
            classification: "Roadmap (Planned)",
            description: "Requires compiling on-device ARM64 worker binary (`llama-rpc-server`) and physical phone connection.",
            is_supported: false,
            default_records: 0,
        },
    ]
}

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Benchmark selection menu & parameters
            Constraint::Length(3), // Running Progress or Execution trigger
            Constraint::Min(8),    // Benchmark Results Table & Comparison
        ])
        .split(area);

    // 1. Benchmark Selection Menu
    render_selection_menu(f, app, chunks[0]);

    // 2. Status / Progress Banner
    render_progress_banner(f, app, chunks[1]);

    // 3. Results Table
    render_results_table(f, app, chunks[2]);
}

fn render_selection_menu(f: &mut Frame, app: &App, area: Rect) {
    let benchmarks = supported_benchmarks();
    let _selected = &benchmarks[app.selected_benchmark_index.min(benchmarks.len() - 1)];

    let mut menu_lines = Vec::new();
    for (i, b) in benchmarks.iter().enumerate() {
        let is_sel = i == app.selected_benchmark_index;
        let prefix = if is_sel { "▶ " } else { "  " };

        let name_style = if !b.is_supported {
            Style::default().fg(COLOR_MUTED)
        } else if is_sel {
            Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)
        } else {
            style_value()
        };

        let tag_style = if !b.is_supported {
            Style::default().fg(COLOR_ERROR)
        } else {
            Style::default().fg(COLOR_SECONDARY)
        };

        menu_lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(COLOR_ACCENT)),
            Span::styled(format!("{:<34} ", b.name), name_style),
            Span::styled(format!("[{:<20}] ", b.classification), tag_style),
            Span::styled(b.description, Style::default().fg(COLOR_MUTED)),
        ]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_border_focused())
        .title(format!(
            " REPRODUCIBLE EXPERIMENT SUITES (↑/↓ to choose, Repetitions: {}, Port: {}) ",
            app.benchmark_runs, app.port
        ));

    f.render_widget(Paragraph::new(menu_lines).block(block), area);
}

fn render_progress_banner(f: &mut Frame, app: &App, area: Rect) {
    let benchmarks = supported_benchmarks();
    let selected = &benchmarks[app.selected_benchmark_index.min(benchmarks.len() - 1)];

    if app.benchmark_running {
        let (done, total) = app.benchmark_progress;
        let ratio = if total > 0 { done as f64 / total as f64 } else { 0.0 };
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).border_style(style_border_focused()))
            .gauge_style(Style::default().fg(COLOR_ACCENT))
            .ratio(ratio)
            .label(format!("{:.0}% — {}", ratio * 100.0, app.benchmark_stage));
        f.render_widget(gauge, area);
    } else if !selected.is_supported {
        let p = Paragraph::new(Line::from(vec![
            badge_err("UNSUPPORTED"),
            Span::raw(" "),
            Span::styled(
                "This benchmark is disabled: Native distributed inference requires upstream llama.cpp RPC integration.",
                Style::default().fg(COLOR_ERROR),
            ),
        ]))
        .block(Block::default().borders(Borders::ALL).border_style(style_border_normal()));
        f.render_widget(p, area);
    } else {
        let p = Paragraph::new(Line::from(vec![
            Span::styled("Ready to execute: ", style_label()),
            Span::styled(selected.name, Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled("  |  Press ", style_label()),
            Span::styled("[Enter]", Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD)),
            Span::styled(" to run experiment  |  Press ", style_label()),
            Span::styled("[+/-]", style_value()),
            Span::styled(" to adjust repetitions ({})  |  Press ", style_label()),
            Span::styled("[s]", style_value()),
            Span::styled(" to save JSON report", style_label()),
        ]))
        .block(Block::default().borders(Borders::ALL).border_style(style_border_normal()));
        f.render_widget(p, area);
    }
}

fn render_results_table(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        "TEST / PAYLOAD SIZE",
        "TEST TYPE",
        "RUNS",
        "MEDIAN TIME",
        "P95 TIME",
        "MIN / MAX",
        "THROUGHPUT",
    ])
    .style(Style::default().fg(COLOR_SECONDARY).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = if app.benchmark_results.is_empty() {
        vec![Row::new(vec![
            Span::styled("No benchmark results in memory. Select a suite above and press [Enter].", Style::default().fg(COLOR_MUTED)),
            Span::raw(""),
            Span::raw(""),
            Span::raw(""),
            Span::raw(""),
            Span::raw(""),
            Span::raw(""),
        ])]
    } else {
        app.benchmark_results
            .iter()
            .map(|r| {
                let size_str = format_bytes(r.payload_size_bytes as u64);
                let tp_str = match r.stats.throughput_mbps {
                    Some(tp) => format!("{:.1} Mbps", tp),
                    None => "-".to_string(),
                };

                Row::new(vec![
                    Span::styled(size_str, style_value()),
                    Span::styled(r.test_type.to_string(), Style::default().fg(COLOR_ACCENT)),
                    Span::styled(format!("{}", r.runs), style_value()),
                    Span::styled(format_duration(r.stats.median_ms), Style::default().fg(COLOR_SUCCESS)),
                    Span::styled(format_duration(r.stats.p95_ms), style_value()),
                    Span::styled(format!("{} / {}", format_duration(r.stats.min_ms), format_duration(r.stats.max_ms)), Style::default().fg(COLOR_MUTED)),
                    Span::styled(tp_str, Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)),
                ])
            })
            .collect()
    };

    let title = if let Some(path) = &app.last_saved_report {
        format!(" BENCHMARK MEASUREMENTS & LATENCY DISTRIBUTIONS (Saved: {}) ", path)
    } else {
        " BENCHMARK MEASUREMENTS & LATENCY DISTRIBUTIONS ".to_string()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(22),
            Constraint::Length(18),
            Constraint::Length(8),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(22),
            Constraint::Length(16),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).border_style(style_border_normal()).title(title));

    f.render_widget(table, area);
}
