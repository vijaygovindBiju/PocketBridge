use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use super::app::{ActionItem, App};
use super::theme::*;

/// Render centered popup modal
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn render_help_modal(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(75, 75, area);
    f.render_widget(Clear, popup_area);

    let lines = vec![
        Line::from(vec![
            Span::styled("POCKETBRIDGE SHORTCUT & COMMAND CHEAT SHEET", Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(Span::styled("─".repeat(popup_area.width.saturating_sub(4) as usize), Style::default().fg(COLOR_MUTED))),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("GLOBAL NAVIGATION:", Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  1 .. 7         ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Jump to screen (Overview, Devices, Tasks, Benchmarks, Scheduler, Plugins, Logs)", style_value()),
        ]),
        Line::from(vec![
            Span::styled("  Tab / S-Tab    ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Next / Previous screen", style_value()),
        ]),
        Line::from(vec![
            Span::styled("  Space          ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Open interactive Action Palette", style_value()),
        ]),
        Line::from(vec![
            Span::styled("  ?              ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Toggle this Help modal", style_value()),
        ]),
        Line::from(vec![
            Span::styled("  q / Ctrl+C     ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Quit application safely (restores terminal state)", style_value()),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("DEVICE & TRANSPORT ACTIONS:", Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  c              ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Setup ADB tcp:9876 forwarding & handshake worker phone", style_value()),
        ]),
        Line::from(vec![
            Span::styled("  d              ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Safely disconnect active worker session", style_value()),
        ]),
        Line::from(vec![
            Span::styled("  r              ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Refresh host & worker telemetry / heartbeat ping", style_value()),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("TASKS & BENCHMARKS:", Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  j              ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Execute fast calibration task (chunk_transform 50k records)", style_value()),
        ]),
        Line::from(vec![
            Span::styled("  Enter          ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Execute selected benchmark or inspect selected item", style_value()),
        ]),
        Line::from(vec![
            Span::styled("  + / -          ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Adjust benchmark repetitions or workload records size", style_value()),
        ]),
        Line::from(vec![
            Span::styled("  s              ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Save reproducible benchmark results to JSON file", style_value()),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("DIAGNOSTICS & SEARCH:", Style::default().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  /              ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Focus log search filter input", style_value()),
        ]),
        Line::from(vec![
            Span::styled("  Esc            ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Close modal or dismiss search filter", style_value()),
        ]),
        Line::from(Span::raw("")),
        Line::from(Span::styled("Press [Esc] or [?] to close this help window.", Style::default().fg(COLOR_MUTED))),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_border_focused())
        .title(" HELP & KEYBOARD REFERENCE ");

    f.render_widget(Paragraph::new(lines).block(block), popup_area);
}

pub fn render_action_palette(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(65, 60, area);
    f.render_widget(Clear, popup_area);

    let query = app.action_palette_query.to_lowercase();
    let all_actions = App::actions_list();
    let filtered: Vec<&ActionItem> = all_actions
        .iter()
        .filter(|a| {
            if query.is_empty() {
                true
            } else {
                a.title.to_lowercase().contains(&query) || a.description.to_lowercase().contains(&query)
            }
        })
        .collect();

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Search Action: ", style_label()),
            Span::styled(format!("{}█", app.action_palette_query), Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(Span::styled("─".repeat(popup_area.width.saturating_sub(4) as usize), Style::default().fg(COLOR_MUTED))),
    ];

    if filtered.is_empty() {
        lines.push(Line::from(Span::styled("No matching commands found.", Style::default().fg(COLOR_MUTED))));
    } else {
        for (idx, action) in filtered.iter().enumerate() {
            let is_sel = idx == app.selected_action_index.min(filtered.len().saturating_sub(1));
            let prefix = if is_sel { "▶ " } else { "  " };

            let title_style = if is_sel {
                Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)
            } else {
                style_value()
            };

            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(COLOR_ACCENT)),
                Span::styled(format!("{:<30} ", action.title), title_style),
                Span::styled(format!("[{:<4}] ", action.shortcut), Style::default().fg(COLOR_SUCCESS)),
                Span::styled(action.description, Style::default().fg(COLOR_MUTED)),
            ]));
        }
    }

    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(Span::styled(
        "Use [↑/↓] to navigate, [Enter] to run action, [Esc] to cancel.",
        Style::default().fg(COLOR_MUTED),
    )));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_border_focused())
        .title(" COMMAND & ACTION PALETTE ");

    f.render_widget(Paragraph::new(lines).block(block), popup_area);
}
