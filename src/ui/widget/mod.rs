use ratatui::{
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, StatefulWidget, Widget},
};

use crate::log::{LogEntry, LogLevel};

pub struct WidgetList<'a, W: Widget + Clone> {
    items: Vec<W>,
    item_height: u16,
    block: Option<Block<'a>>,
}
impl<'a, W: Widget + Clone> WidgetList<'a, W> {
    pub fn new(items: Vec<W>, item_height: u16) -> Self {
        Self {
            items,
            item_height,
            block: None,
        }
    }
    pub fn block(mut self, block: Option<Block<'a>>) -> Self {
        self.block = block;
        self
    }
}

/// State for a [`WidgetList`], tracking the scroll offset and current selection.
///
/// Like ratatui's own `ListState`, this keeps its fields private and exposes
/// them through methods. The `offset` is the index of the first item that
/// should be drawn; the widget adjusts it during rendering so that the
/// selected item stays visible.
///
/// The navigation methods (`select_next`, `select_last`, etc.) take the number
/// of items as a `len` argument, since the state itself does not know how many
/// items the list contains.
///
/// # Examples
///
/// ```
/// let mut state = WidgetListState::new();
/// state.select_first(items.len());
/// state.select_next(items.len());
/// assert_eq!(state.selected(), Some(1));
/// ```
#[derive(Debug, Default, Clone)]
pub struct WidgetListState {
    /// Index of the first visible item.
    offset: usize,
    /// Index of the currently selected item, if any.
    selected: Option<usize>,
}

impl WidgetListState {
    /// Creates a new state with no selection and a zero offset.
    pub fn new() -> Self {
        Self::default()
    }

    // --- offset ---

    /// Returns the current scroll offset (the index of the first visible item).
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns a mutable reference to the scroll offset.
    ///
    /// Prefer the navigation methods where possible; use this only when you
    /// need to set the offset directly.
    pub fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }

    /// Sets the scroll offset, consuming and returning `self` for chaining.
    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    // --- selection ---

    /// Returns the index of the selected item, or `None` if nothing is selected.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Sets the selected item.
    ///
    /// Passing `None` clears the selection and resets the offset to `0`,
    /// matching ratatui's behaviour.
    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index;
        if index.is_none() {
            self.offset = 0;
        }
    }

    /// Sets the selected item, consuming and returning `self` for chaining.
    pub fn with_selected(mut self, index: Option<usize>) -> Self {
        self.select(index);
        self
    }

    // --- navigation ---

    /// Selects the next item, clamped to the last item.
    ///
    /// If nothing is selected, selects the first item. `len` is the number of
    /// items in the list; if it is `0`, the selection is cleared.
    pub fn select_next(&mut self, len: usize) {
        if len == 0 {
            self.selected = None;
            return;
        }
        let next = match self.selected {
            Some(i) => (i + 1).min(len - 1),
            None => 0,
        };
        self.selected = Some(next);
    }

    /// Selects the previous item, clamped to the first item.
    ///
    /// If nothing is selected, selects the first item. `len` is the number of
    /// items in the list; if it is `0`, the selection is cleared.
    pub fn select_previous(&mut self, len: usize) {
        if len == 0 {
            self.selected = None;
            return;
        }
        let prev = match self.selected {
            Some(i) => i.saturating_sub(1),
            None => 0,
        };
        self.selected = Some(prev);
    }

    /// Selects the first item, or clears the selection if the list is empty.
    pub fn select_first(&mut self, len: usize) {
        self.selected = (len > 0).then_some(0);
    }

    /// Selects the last item, or clears the selection if the list is empty.
    pub fn select_last(&mut self, len: usize) {
        self.selected = len.checked_sub(1);
    }

    /// Moves the selection down by `amount`, clamped to the last item.
    ///
    /// If nothing is selected, selects the first item. `len` is the number of
    /// items in the list; if it is `0`, the selection is cleared.
    pub fn scroll_down_by(&mut self, amount: usize, len: usize) {
        if len == 0 {
            self.selected = None;
            return;
        }
        let next = match self.selected {
            Some(i) => (i + amount).min(len - 1),
            None => 0,
        };
        self.selected = Some(next);
    }

    /// Moves the selection up by `amount`, clamped to the first item.
    ///
    /// If nothing is selected, selects the first item.
    pub fn scroll_up_by(&mut self, amount: usize) {
        let prev = match self.selected {
            Some(i) => i.saturating_sub(amount),
            None => 0,
        };
        self.selected = Some(prev);
    }
}

impl<'a, W: Widget + Clone> StatefulWidget for WidgetList<'a, W> {
    type State = WidgetListState;

    fn render(self, mut area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if let Some(block) = self.block {
            let inner_area = block.inner(area);
            block.render(area, buf);
            area = inner_area;
        }

        // Guard against a zero item height (would cause div-by-zero / no progress).
        if self.item_height == 0 || area.height == 0 {
            return;
        }

        // How many items fit fully in the visible area.
        let visible_count = (area.height / self.item_height) as usize;
        if visible_count == 0 {
            return;
        }

        // Clamp offset to a valid range for the current item count.
        let max_offset = self.items.len().saturating_sub(visible_count);
        state.offset = state.offset.min(max_offset);

        // Adjust the offset so the selected item stays visible.
        if let Some(selected) = state.selected {
            let selected = selected.min(self.items.len().saturating_sub(1));
            if selected < state.offset {
                // selection scrolled off the top
                state.offset = selected;
            } else if selected >= state.offset + visible_count {
                // selection scrolled off the bottom
                state.offset = selected + 1 - visible_count;
            }
        }

        let mut y = area.y;
        for (i, item) in self.items.iter().enumerate().skip(state.offset) {
            // stop if we've run out of vertical room
            if y + self.item_height > area.y + area.height {
                break;
            }

            let item_area = Rect {
                x: area.x,
                y,
                width: area.width,
                height: self.item_height,
            };

            item.clone().render(item_area, buf);

            // Optionally highlight the selected row.
            if Some(i) == state.selected {
                buf.set_style(item_area, Style::default().add_modifier(Modifier::REVERSED));
            }

            y += self.item_height;
        }
    }
}

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

