//! Shared Ratatui widget helpers.

use ratatui::{
    layout::Alignment,
    style::{Color, Style},
    widgets::{Block, BorderType},
};

/// Returns a rounded border block styled to indicate focus state.
///
/// Yellow border when focused, dark gray otherwise.
pub(crate) fn focused_block(title: &'static str, focused: bool) -> Block<'static> {
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Block::bordered()
        .title(title)
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
}
