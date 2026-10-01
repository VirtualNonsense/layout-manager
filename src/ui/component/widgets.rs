//! Shared Ratatui widget helpers.

use ratatui::widgets::Block;

use crate::ui_lib::theme::DEFAULT_THEME;

/// Returns a rounded border block styled to indicate focus state.
///
/// Yellow border when focused, dark gray otherwise.
pub(crate) fn focused_block(title: &'static str, focused: bool) -> Block<'static> {
    if focused {
        DEFAULT_THEME.focused_block(title)
    } else {
        DEFAULT_THEME.block(title)
    }
}
