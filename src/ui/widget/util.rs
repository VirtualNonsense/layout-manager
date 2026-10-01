use ratatui::{
    prelude::*,
    widgets::{Paragraph, Wrap},
};

use crate::ui_lib::theme::Theme;

pub fn render_centered_text(text: &str, style: Style, area: Rect, buffer: &mut Buffer) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let areas = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .split(area);

    Paragraph::new(text)
        .style(style)
        .alignment(Alignment::Center)
        .render(areas[1], buffer);
}

pub fn render_error(title: &str, details: &str, theme: &Theme, area: Rect, buffer: &mut Buffer) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let areas = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .split(area);
    let message = Text::from(vec![
        Line::from(Span::styled(title, theme.error)),
        Line::from(""),
        Line::from(vec![
            Span::styled("Details: ", theme.label),
            Span::styled(details, theme.value),
        ]),
    ]);

    Paragraph::new(message)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center)
        .render(areas[1], buffer);
}
