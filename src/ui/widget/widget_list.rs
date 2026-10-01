use ratatui::{
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, StatefulWidget, Widget},
};

/// A widget that reports how many terminal rows it requires.
///
/// The height may change between render calls. This allows widgets such as
/// expandable log entries to work inside WidgetList.
pub trait HeightAwareWidget: Widget + Clone {
    /// Returns the desired widget height in terminal rows.
    ///
    /// WidgetList treats a returned value of zero as one row.
    fn height(&self) -> u16;
}

/// A generic, selectable list of variable-height widgets.
pub struct WidgetList<'a, W>
where
    W: HeightAwareWidget,
{
    items: &'a [W],
    block: Option<Block<'a>>,
    highlight_style: Style,
    highlight_full_item: bool,
}

impl<'a, W> WidgetList<'a, W>
where
    W: HeightAwareWidget,
{
    /// Creates a widget list containing the supplied items.
    pub const fn new(items: &'a [W]) -> Self {
        Self {
            items,
            block: None,
            highlight_style: Style::new()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            highlight_full_item: false,
        }
    }

    /// Adds an optional block around the list.
    pub fn block(mut self, block: Option<Block<'a>>) -> Self {
        self.block = block;
        self
    }

    /// Sets the style used for the selected item.
    pub const fn highlight_style(mut self, style: Style) -> Self {
        self.highlight_style = style;
        self
    }

    /// Controls whether selection highlighting covers the complete item.
    ///
    /// When false, only the first row is highlighted. This is useful for
    /// expandable items whose additional rows contain detailed information.
    ///
    /// When true, every visible row belonging to the item is highlighted.
    pub const fn highlight_full_item(mut self, highlight_full_item: bool) -> Self {
        self.highlight_full_item = highlight_full_item;
        self
    }
}

/// State for a WidgetList.
///
/// The state tracks the selected item and the index of the first visible item.
#[derive(Debug, Default, Clone)]
pub struct WidgetListState {
    /// Index of the first visible item.
    offset: usize,

    /// Index of the selected item.
    selected: Option<usize>,
}

impl WidgetListState {
    /// Creates a state with no selection and an offset of zero.
    pub const fn new() -> Self {
        Self {
            offset: 0,
            selected: None,
        }
    }

    /// Returns the index of the first visible item.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns a mutable reference to the current offset.
    pub fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }

    /// Sets the offset and returns the updated state.
    pub const fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    /// Returns the selected item index.
    pub const fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Sets the selected item.
    ///
    /// Clearing the selection also resets the scroll offset.
    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index;

        if index.is_none() {
            self.offset = 0;
        }
    }

    /// Sets the selected item and returns the updated state.
    pub fn with_selected(mut self, index: Option<usize>) -> Self {
        self.select(index);
        self
    }

    /// Selects the next item without wrapping.
    pub fn select_next(&mut self, len: usize) {
        if len == 0 {
            self.selected = None;
            self.offset = 0;
            return;
        }

        let next = match self.selected {
            Some(index) => index.saturating_add(1).min(len - 1),
            None => 0,
        };

        self.selected = Some(next);
    }

    /// Selects the previous item without wrapping.
    pub fn select_previous(&mut self, len: usize) {
        if len == 0 {
            self.selected = None;
            self.offset = 0;
            return;
        }

        let previous = match self.selected {
            Some(index) => index.saturating_sub(1),
            None => 0,
        };

        self.selected = Some(previous);
    }

    /// Selects the first item.
    pub fn select_first(&mut self, len: usize) {
        self.selected = (len > 0).then_some(0);

        if len == 0 {
            self.offset = 0;
        }
    }

    /// Selects the final item.
    pub fn select_last(&mut self, len: usize) {
        self.selected = len.checked_sub(1);

        if len == 0 {
            self.offset = 0;
        }
    }

    /// Moves the selection down by the supplied number of items.
    pub fn scroll_down_by(&mut self, amount: usize, len: usize) {
        if len == 0 {
            self.selected = None;
            self.offset = 0;
            return;
        }

        let next = match self.selected {
            Some(index) => index.saturating_add(amount).min(len - 1),
            None => 0,
        };

        self.selected = Some(next);
    }

    /// Moves the selection up by the supplied number of items.
    pub fn scroll_up_by(&mut self, amount: usize) {
        let previous = match self.selected {
            Some(index) => index.saturating_sub(amount),
            None => 0,
        };

        self.selected = Some(previous);
    }

    /// Clears the current selection.
    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.offset = 0;
    }

    /// Ensures selection and offset refer to valid items.
    fn normalize(&mut self, len: usize) {
        if len == 0 {
            self.selected = None;
            self.offset = 0;
            return;
        }

        if let Some(selected) = self.selected {
            self.selected = Some(selected.min(len - 1));
        }

        self.offset = self.offset.min(len - 1);
    }
}

impl<'a, W> StatefulWidget for WidgetList<'a, W>
where
    W: HeightAwareWidget,
{
    type State = WidgetListState;

    fn render(self, mut area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let WidgetList {
            items,
            block,
            highlight_style,
            highlight_full_item,
        } = self;

        if let Some(block) = block {
            let inner_area = block.inner(area);
            block.render(area, buffer);
            area = inner_area;
        }

        if area.width == 0 || area.height == 0 {
            return;
        }

        state.normalize(items.len());

        if items.is_empty() {
            return;
        }

        ensure_selection_visible(items, area.height, state);

        let bottom = area.y.saturating_add(area.height);
        let mut y = area.y;

        for (index, item) in items.iter().enumerate().skip(state.offset) {
            if y >= bottom {
                break;
            }

            let requested_height = item.height().max(1);
            let available_height = bottom.saturating_sub(y);
            let rendered_height = requested_height.min(available_height);

            let item_area = Rect {
                x: area.x,
                y,
                width: area.width,
                height: rendered_height,
            };

            item.clone().render(item_area, buffer);

            if Some(index) == state.selected {
                let highlight_height = if highlight_full_item {
                    rendered_height
                } else {
                    rendered_height.min(1)
                };

                if highlight_height > 0 {
                    let highlight_area = Rect {
                        x: item_area.x,
                        y: item_area.y,
                        width: item_area.width,
                        height: highlight_height,
                    };

                    buffer.set_style(highlight_area, highlight_style);
                }
            }

            if rendered_height < requested_height {
                break;
            }

            y = y.saturating_add(requested_height);
        }
    }
}

/// Adjusts the offset so the selected item is visible.
///
/// Heights are accumulated dynamically because individual items may occupy
/// different numbers of terminal rows.
fn ensure_selection_visible<W>(items: &[W], available_height: u16, state: &mut WidgetListState)
where
    W: HeightAwareWidget,
{
    let Some(selected) = state.selected else {
        return;
    };

    if selected < state.offset {
        state.offset = selected;
    }

    loop {
        let used_height = items[state.offset..=selected]
            .iter()
            .fold(0_u16, |height, item| {
                height.saturating_add(item.height().max(1))
            });

        if used_height <= available_height || state.offset >= selected {
            break;
        }

        state.offset = state.offset.saturating_add(1);
    }
}
