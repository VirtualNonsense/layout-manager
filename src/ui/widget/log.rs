use ratatui::{
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::log::{LogEntry, LogLevel};

impl LogLevel {
    /// The accent color used for this level's badge.
    fn color(self) -> Color {
        match self {
            LogLevel::Trace => Color::DarkGray,
            LogLevel::Debug => Color::Blue,
            LogLevel::Info => Color::Green,
            LogLevel::Warn => Color::Yellow,
            LogLevel::Error => Color::Red,
        }
    }
}

impl LogEntry {
    /// Build the styled `Line` for this entry. Shared by both the owned and
    /// by-reference `Widget` impls so there's a single source of truth.
    fn to_line(&self) -> Line<'_> {
        let mut spans: Vec<Span> = vec![
            // Timestamp — dimmed, so the eye lands on level + message first.
            Span::styled(
                self.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(" "),
            // Level badge — bold, color-coded.
            Span::styled(
                self.level.label(),
                Style::default()
                    .fg(self.level.color())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ];

        // Span chain (if any), rendered as `outer›inner:` in a muted color.
        if !self.spans.is_empty() {
            spans.push(Span::styled(
                format!("{}: ", self.spans.join("›")),
                Style::default().fg(Color::Cyan),
            ));
        }

        // The message itself.
        spans.push(Span::raw(self.message.as_str()));

        Line::from(spans)
    }
}

/// Owned rendering — consumes the entry (matches the signature you started with).
impl Widget for LogEntry {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        // Render into the first row of `area`; ratatui truncates to width
        // automatically. Use a Paragraph if you'd rather wrap.
        buf.set_line(area.x, area.y, &self.to_line(), area.width);
    }
}
