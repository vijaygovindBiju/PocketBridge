use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use super::app::{App, LogSeverity};
use super::theme::*;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search & Filter Header
            Constraint::Min(8),    // Logs Stream
        ])
        .split(area);

    // 1. Search Bar & Severity Filter
    render_filter_bar(f, app, chunks[0]);

    // 2. Logs Stream
    render_logs_stream(f, app, chunks[1]);
}

fn render_filter_bar(f: &mut Frame, app: &App, area: Rect) {
    let query_display = if app.is_searching_logs {
        format!("{}█", app.log_search_query)
    } else if app.log_search_query.is_empty() {
        "(None - Press '/' to search)".to_string()
    } else {
        app.log_search_query.clone()
    };

    let (filter_name, filter_color) = match app.log_severity_filter {
        None => ("ALL", COLOR_FG),
        Some(LogSeverity::Info) => ("INFO ONLY", COLOR_INFO),
        Some(LogSeverity::Warn) => ("WARN ONLY", COLOR_WARNING),
        Some(LogSeverity::Error) => ("ERRORS ONLY", COLOR_ERROR),
        Some(LogSeverity::Debug) => ("DEBUG ONLY", COLOR_MUTED),
    };

    let line = Line::from(vec![
        Span::styled("Search Filter: ", style_label()),
        Span::styled(query_display, if app.is_searching_logs { Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD) } else { style_value() }),
        Span::styled("    Severity Filter: ", style_label()),
        Span::styled(format!("[{}]", filter_name), Style::default().fg(filter_color).add_modifier(Modifier::BOLD)),
        Span::styled("    Controls: ", style_label()),
        Span::styled("[/] Search  [Esc] Clear  [F] Toggle Severity  [c] Clear Logs", Style::default().fg(COLOR_MUTED)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(if app.is_searching_logs { style_border_focused() } else { style_border_normal() })
        .title(" DIAGNOSTICS & EVENT STREAM FILTER ");

    f.render_widget(Paragraph::new(line).block(block), area);
}

fn render_logs_stream(f: &mut Frame, app: &App, area: Rect) {
    let query = app.log_search_query.to_lowercase();

    let filtered: Vec<&super::app::LogEntry> = app
        .logs
        .iter()
        .filter(|e| {
            if let Some(sev) = app.log_severity_filter {
                if e.severity != sev {
                    return false;
                }
            }
            if !query.is_empty() {
                let text = format!("{} {} {}", e.component, e.message, e.timestamp).to_lowercase();
                if !text.contains(&query) {
                    return false;
                }
            }
            true
        })
        .collect();

    let visible_capacity = (area.height.saturating_sub(2)) as usize;
    let total = filtered.len();

    // Scroll handling: display the most recent entries that fit in visible capacity
    let start_idx = total.saturating_sub(visible_capacity);
    let slice = if start_idx < total { &filtered[start_idx..] } else { &[] };

    let lines: Vec<Line> = if slice.is_empty() {
        vec![Line::from(Span::styled("No log events matching current filter.", Style::default().fg(COLOR_MUTED)))]
    } else {
        slice
            .iter()
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
            .collect()
    };

    let title = format!(" SYSTEM EVENTS (Showing {} of {} total events) ", slice.len(), app.logs.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_border_normal())
        .title(title);

    f.render_widget(Paragraph::new(lines).block(block), area);
}
