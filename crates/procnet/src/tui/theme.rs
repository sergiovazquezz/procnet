use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use super::state::Unit;

/// 16-color ANSI palette used across the TUI. Sticking to the basic set for
/// portability across terminals.
pub mod color {
    use ratatui::style::Color;
    pub const ACCENT: Color = Color::Cyan;
    pub const MUTED: Color = Color::DarkGray;
    pub const OK: Color = Color::Green;
    pub const WARN: Color = Color::Yellow;
    pub const HOT: Color = Color::Red;
    pub const SENT: Color = Color::Yellow;
    pub const RECV: Color = Color::Green;
    pub const INVERT_TEXT: Color = Color::Black;
}

/// Format an unsigned byte count as a compact human-readable string.
///
/// `Unit::Auto` picks a unit per value (mixed units across rows); a fixed
/// unit divides every value by that unit's divisor so all rows share one
/// unit. Presentation only — does not touch the underlying data model.
pub fn format_bytes(n: u64, unit: Unit) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    const TB: u64 = 1024 * 1024 * 1024 * 1024;

    let value = n as f64;

    match unit {
        Unit::Auto => {
            if n < KB {
                format!("{n} B")
            } else if n < MB {
                format!("{:.2} KB", value / KB as f64)
            } else if n < GB {
                format!("{:.2} MB", value / MB as f64)
            } else if n < TB {
                format!("{:.2} GB", value / GB as f64)
            } else {
                format!("{:.2} TB", value / TB as f64)
            }
        }
        Unit::B => format!("{n} B"),
        fixed => format!("{:.2} {}", value / fixed.divisor() as f64, fixed.label()),
    }
}

/// Map a 0.0..=1.0 ratio (of a row's total against the visible max) to a
/// traffic-light color: red above 66%, yellow between 33% and 66%, green
/// otherwise. Values outside 0..=1 are clamped.
pub fn traffic_color(ratio: f64) -> Color {
    let r = ratio.clamp(0.0, 1.0);
    if r > 0.66 {
        color::HOT
    } else if r > 0.33 {
        color::WARN
    } else {
        color::OK
    }
}

/// A key glyph shown as `[key]`. When `active`, it is rendered with an accent
/// background so the current state is visible at a glance.
pub fn key_span(key: &str, active: bool) -> Span<'static> {
    let style = if active {
        Style::new()
            .bg(color::ACCENT)
            .fg(color::INVERT_TEXT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(color::ACCENT).add_modifier(Modifier::BOLD)
    };
    Span::styled(format!("[{key}]"), style)
}

/// A muted label following a key chip.
pub fn label_span(label: &str) -> Span<'static> {
    Span::styled(label.to_string(), Style::new().fg(color::MUTED))
}

/// A ` │ ` separator between keybind groups.
pub fn sep_span() -> Span<'static> {
    Span::styled(" │ ".to_string(), Style::new().fg(color::MUTED))
}

/// A span in the muted color.
pub fn muted_span(s: &str) -> Span<'static> {
    Span::styled(s.to_string(), Style::new().fg(color::MUTED))
}

/// A span in the accent color.
pub fn accent_span(s: &str) -> Span<'static> {
    Span::styled(s.to_string(), Style::new().fg(color::ACCENT))
}
