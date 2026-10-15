#![allow(dead_code)]
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

// ----------------------------------------------------------------------------
// Restrained Linux Systems Color Palette
// ----------------------------------------------------------------------------

pub const COLOR_BG: Color = Color::Reset;
pub const COLOR_FG: Color = Color::White;
pub const COLOR_MUTED: Color = Color::DarkGray;
pub const COLOR_SECONDARY: Color = Color::Gray;

pub const COLOR_ACCENT: Color = Color::Cyan;
pub const COLOR_SUCCESS: Color = Color::Green;
pub const COLOR_WARNING: Color = Color::Yellow;
pub const COLOR_ERROR: Color = Color::Red;
pub const COLOR_INFO: Color = Color::Blue;

pub fn style_header() -> Style {
    Style::default().fg(COLOR_FG).bg(Color::Reset).add_modifier(Modifier::BOLD)
}

pub fn style_active_tab() -> Style {
    Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)
}

pub fn style_inactive_tab() -> Style {
    Style::default().fg(COLOR_MUTED)
}

pub fn style_label() -> Style {
    Style::default().fg(COLOR_SECONDARY)
}

pub fn style_value() -> Style {
    Style::default().fg(COLOR_FG)
}

pub fn style_selected() -> Style {
    Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::REVERSED | Modifier::BOLD)
}

pub fn style_border_focused() -> Style {
    Style::default().fg(COLOR_ACCENT)
}

pub fn style_border_normal() -> Style {
    Style::default().fg(COLOR_MUTED)
}

// ----------------------------------------------------------------------------
// Visual Formatting & Badges
// ----------------------------------------------------------------------------

pub fn badge_ok(text: &str) -> Span<'static> {
    Span::styled(format!(" {text} "), Style::default().fg(Color::Black).bg(COLOR_SUCCESS).add_modifier(Modifier::BOLD))
}

pub fn badge_warn(text: &str) -> Span<'static> {
    Span::styled(format!(" {text} "), Style::default().fg(Color::Black).bg(COLOR_WARNING).add_modifier(Modifier::BOLD))
}

pub fn badge_err(text: &str) -> Span<'static> {
    Span::styled(format!(" {text} "), Style::default().fg(Color::White).bg(COLOR_ERROR).add_modifier(Modifier::BOLD))
}

pub fn badge_neutral(text: &str) -> Span<'static> {
    Span::styled(format!(" {text} "), Style::default().fg(Color::White).bg(Color::DarkGray))
}

pub fn badge_accent(text: &str) -> Span<'static> {
    Span::styled(format!(" {text} "), Style::default().fg(Color::Black).bg(COLOR_ACCENT).add_modifier(Modifier::BOLD))
}

pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;

    if bytes >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_duration(ms: f64) -> String {
    if ms >= 1000.0 {
        format!("{:.2} s", ms / 1000.0)
    } else {
        format!("{:.1} ms", ms)
    }
}

pub fn render_mini_bar(ratio: f64, width: usize) -> Span<'static> {
    let clamped = ratio.clamp(0.0, 1.0);
    let filled = (clamped * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);

    let color = if clamped > 0.85 {
        COLOR_ERROR
    } else if clamped > 0.65 {
        COLOR_WARNING
    } else {
        COLOR_SUCCESS
    };

    Span::styled(format!("[{}{}]", "█".repeat(filled), "░".repeat(empty)), Style::default().fg(color))
}
