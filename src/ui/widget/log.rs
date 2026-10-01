use std::collections::HashSet;

use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Paragraph, StatefulWidget, Widget},
};
use serde_json::Value;
use tracing::{debug, trace};

use crate::{
    log::{LogEntry, LogLevel, LogSpan},
    ui::widget::{WidgetList, WidgetListState, widget_list::HeightAwareWidget},
    ui_lib::theme::{DEFAULT_THEME, Theme},
};

/// Maximum number of characters displayed for a field value in the preview.
const PREVIEW_VALUE_LENGTH: usize = 32;

/// Number of spaces used to indent expanded details.
const DETAIL_INDENT: &str = "    ";

/// State for the complete log view.
///
/// `WidgetListState` handles selection and scrolling. The expanded set contains
/// the indices of log entries whose structured details are visible.
///
/// `list_area` contains the area occupied by the actual list content during the
/// most recent render. It excludes the surrounding block and is used to
/// translate pointer positions into entry indices.
#[derive(Debug, Default, Clone)]
pub struct LogViewState {
    pub list: WidgetListState,
    expanded: HashSet<usize>,
    list_area: Rect,
}

impl LogViewState {
    /// Creates an empty log-view state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the index of the selected log entry.
    pub fn selected(&self) -> Option<usize> {
        self.list.selected()
    }

    /// Selects the supplied log entry.
    pub fn select(&mut self, index: usize) {
        self.list.select(Some(index));
    }

    /// Returns whether the supplied entry is expanded.
    pub fn is_expanded(&self, index: usize) -> bool {
        self.expanded.contains(&index)
    }

    /// Returns the area occupied by the list during the most recent render.
    pub const fn list_area(&self) -> Rect {
        self.list_area
    }

    /// Updates the area occupied by the rendered list.
    pub fn set_list_area(&mut self, area: Rect) {
        self.list_area = area;
    }

    /// Returns whether the supplied pointer position is inside the list.
    pub fn list_area_contains(&self, position: Position) -> bool {
        self.list_area.contains(position)
    }

    /// Returns the log-entry index at the supplied pointer position.
    ///
    /// This accounts for the current scroll offset and the variable height of
    /// expanded log entries.
    pub fn index_at_pointer(&self, position: Position, entries: &[LogEntry]) -> Option<usize> {
        if self.list_area.is_empty() {
            trace!("cannot resolve log pointer index because the list area is empty");
            return None;
        }

        if !self.list_area.contains(position) {
            trace!(
                ?position,
                area = ?self.list_area,
                "pointer position is outside the log list"
            );
            return None;
        }

        let relative_y = position.y.saturating_sub(self.list_area.y);
        let first_visible_index = self.list.offset();
        let mut current_y = 0_u16;

        for (index, entry) in entries.iter().enumerate().skip(first_visible_index) {
            let item_height = LogEntryWidget::new(entry, self.is_expanded(index)).height();

            let item_end = current_y.saturating_add(item_height);

            if relative_y >= current_y && relative_y < item_end {
                return Some(index);
            }

            current_y = item_end;

            if current_y >= self.list_area.height {
                break;
            }
        }

        None
    }

    /// Selects the log entry at the supplied pointer position.
    ///
    /// Returns the selected index when the pointer references a visible entry.
    pub fn select_at_pointer(&mut self, position: Position, entries: &[LogEntry]) -> Option<usize> {
        let index = self.index_at_pointer(position, entries)?;

        self.select(index);

        trace!(index, "selected log item using pointer");

        Some(index)
    }

    /// Selects and toggles the log entry at the supplied pointer position.
    ///
    /// Returns the selected index when the pointer references a visible entry.
    pub fn toggle_at_pointer(&mut self, position: Position, entries: &[LogEntry]) -> Option<usize> {
        let index = self.select_at_pointer(position, entries)?;

        self.toggle(index);

        Some(index)
    }

    /// Expands or collapses the supplied entry.
    pub fn toggle(&mut self, index: usize) {
        if self.expanded.remove(&index) {
            trace!(index, "collapsed log item");
        } else {
            self.expanded.insert(index);
            trace!(index, "expanded log item");
        }
    }

    /// Expands or collapses the selected entry.
    pub fn toggle_selected(&mut self) {
        let Some(selected) = self.selected() else {
            return;
        };

        self.toggle(selected);
    }

    /// Expands the selected entry.
    pub fn expand_selected(&mut self) {
        let Some(selected) = self.selected() else {
            return;
        };

        if self.expanded.insert(selected) {
            trace!(index = selected, "expanded log item");
        }
    }

    /// Collapses the selected entry.
    pub fn collapse_selected(&mut self) {
        let Some(selected) = self.selected() else {
            return;
        };

        if self.expanded.remove(&selected) {
            trace!(index = selected, "collapsed log item");
        }
    }

    /// Collapses all expanded entries.
    pub fn collapse_all(&mut self) {
        if !self.expanded.is_empty() {
            trace!("collapsing all log entries");
            self.expanded.clear();
        }
    }

    /// Removes state referring to entries that no longer exist.
    pub fn normalize(&mut self, entry_count: usize) {
        self.expanded.retain(|index| *index < entry_count);

        if entry_count == 0 {
            self.list.select(None);
            return;
        }

        if let Some(selected) = self.selected() {
            self.list.select(Some(selected.min(entry_count - 1)));
        }
    }
}

/// Stateful widget that renders a complete log view.
///
/// The widget borrows the log data from the component. Persistent UI state,
/// such as selection, scrolling, and expanded entries, is stored separately in
/// `LogViewState`.
pub struct LogViewWidget<'a> {
    entries: &'a [LogEntry],
    block: Option<Block<'a>>,
    updates_paused: bool,
    update_pending: bool,
    theme: &'a Theme,
}

impl<'a> LogViewWidget<'a> {
    /// Creates a log-view widget using the default application theme.
    pub const fn new(entries: &'a [LogEntry]) -> Self {
        Self {
            entries,
            block: None,
            updates_paused: false,
            update_pending: false,
            theme: &DEFAULT_THEME,
        }
    }

    /// Sets the block rendered around the log view.
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Sets whether automatic log updates are paused.
    pub const fn updates_paused(mut self, paused: bool) -> Self {
        self.updates_paused = paused;
        self
    }

    /// Sets whether a log update is currently being fetched.
    pub const fn update_pending(mut self, pending: bool) -> Self {
        self.update_pending = pending;
        self
    }

    /// Sets the theme used by the log view.
    pub const fn theme(mut self, theme: &'a Theme) -> Self {
        self.theme = theme;
        self
    }

    fn empty_message(&self) -> &'static str {
        if self.updates_paused {
            "log updates paused"
        } else if self.update_pending {
            "fetching logs"
        } else {
            "no logs available"
        }
    }

    fn empty_message_style(&self) -> Style {
        if self.updates_paused {
            self.theme.accent
        } else {
            self.theme.muted
        }
    }
}

impl StatefulWidget for LogViewWidget<'_> {
    type State = LogViewState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.normalize(self.entries.len());

        let list_area = self.block.as_ref().map_or(area, |block| block.inner(area));

        state.set_list_area(list_area);

        if self.entries.is_empty() {
            state.set_list_area(Rect::default());

            let paragraph = Paragraph::new(self.empty_message()).style(self.empty_message_style());

            if let Some(block) = self.block {
                paragraph.block(block).render(area, buffer);
            } else {
                paragraph.render(area, buffer);
            }

            return;
        }

        let entry_widgets = self
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                LogEntryWidget::with_theme(entry, state.is_expanded(index), self.theme)
            })
            .collect::<Vec<_>>();

        let mut list = WidgetList::new(&entry_widgets)
            .highlight_style(self.theme.selected)
            .highlight_full_item(false);

        if let Some(block) = self.block {
            list = list.block(Some(block));
        }

        list.render(area, buffer, &mut state.list);
    }
}

/// Rendering wrapper for one `LogEntry`.
///
/// The wrapper separates display state from the parsed log data. Multiple
/// widgets may therefore reference the same `LogEntry` with different display
/// settings or themes.
#[derive(Clone, Copy)]
pub struct LogEntryWidget<'a> {
    entry: &'a LogEntry,
    expanded: bool,
    theme: &'a Theme,
}

impl<'a> LogEntryWidget<'a> {
    /// Creates a log-entry widget using the default application theme.
    pub const fn new(entry: &'a LogEntry, expanded: bool) -> Self {
        Self {
            entry,
            expanded,
            theme: &DEFAULT_THEME,
        }
    }

    /// Creates a log-entry widget using the supplied theme.
    pub const fn with_theme(entry: &'a LogEntry, expanded: bool, theme: &'a Theme) -> Self {
        Self {
            entry,
            expanded,
            theme,
        }
    }

    /// Returns the referenced log entry.
    pub const fn entry(&self) -> &'a LogEntry {
        self.entry
    }

    /// Returns whether the widget is expanded.
    pub const fn expanded(&self) -> bool {
        self.expanded
    }

    /// Returns the theme used to render the entry.
    pub const fn theme(&self) -> &'a Theme {
        self.theme
    }
}

impl HeightAwareWidget for LogEntryWidget<'_> {
    fn height(&self) -> u16 {
        if !self.expanded {
            return 1;
        }

        let detail_height = u16::try_from(self.entry.detail_line_count()).unwrap_or(u16::MAX);

        1_u16.saturating_add(detail_height)
    }
}

impl Widget for LogEntryWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let preview_line = self.entry.preview_line(self.expanded, self.theme);

        buffer.set_line(area.x, area.y, &preview_line, area.width);

        if !self.expanded || area.height <= 1 {
            return;
        }

        let bottom = area.y.saturating_add(area.height);
        let mut y = area.y.saturating_add(1);

        for detail_line in self.entry.detail_lines(self.theme) {
            if y >= bottom {
                break;
            }

            buffer.set_line(area.x, y, &detail_line, area.width);
            y = y.saturating_add(1);
        }
    }
}

impl LogLevel {
    /// Returns the semantic theme style for this log level.
    fn style(self, theme: &Theme) -> Style {
        match self {
            Self::Trace => theme.muted,
            Self::Debug => theme.debug,
            Self::Info => theme.info,
            Self::Warn => theme.warning,
            Self::Error => theme.error,
        }
    }
}

impl LogEntry {
    /// Builds the compact preview line for this entry.
    fn preview_line(&self, expanded: bool, theme: &Theme) -> Line<'_> {
        let marker = if expanded { "▼ " } else { "▶ " };

        let mut spans = vec![
            Span::styled(marker, theme.muted),
            Span::styled(
                self.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                theme.muted,
            ),
            Span::raw(" "),
            Span::styled(self.level.label(), self.level.style(theme)),
            Span::raw(" "),
        ];

        if !self.spans.is_empty() {
            spans.push(Span::styled(
                format!("{}: ", self.span_path()),
                theme.accent,
            ));
        }

        spans.push(Span::styled(self.message.as_str(), theme.text));

        for (key, value) in &self.fields {
            let formatted_value = format_value(value);
            let preview_value = truncate_chars(&formatted_value, PREVIEW_VALUE_LENGTH);

            spans.push(Span::raw(" "));
            spans.push(Span::styled(format!("{key}="), theme.label));
            spans.push(Span::styled(preview_value, theme.data));
        }

        Line::from(spans)
    }

    /// Returns active span names in outermost-to-innermost order.
    fn span_path(&self) -> String {
        self.spans
            .iter()
            .map(|span| span.name.as_str())
            .collect::<Vec<_>>()
            .join("›")
    }

    /// Returns the number of detail lines shown when expanded.
    fn detail_line_count(&self) -> usize {
        let target_line_count = usize::from(!self.target.is_empty());

        let event_field_line_count = if self.fields.is_empty() {
            1
        } else {
            self.fields.len()
        };

        let span_line_count = self
            .spans
            .iter()
            .map(|span| 1_usize.saturating_add(span.fields.len()))
            .sum::<usize>();

        target_line_count
            .saturating_add(event_field_line_count)
            .saturating_add(span_line_count)
    }

    /// Builds the lines displayed beneath an expanded preview.
    fn detail_lines(&self, theme: &Theme) -> Vec<Line<'static>> {
        let mut lines = Vec::with_capacity(self.detail_line_count());

        if !self.target.is_empty() {
            lines.push(detail_line(
                "target",
                self.target.clone(),
                theme,
                DetailValueStyle::Info,
            ));
        }

        if self.fields.is_empty() {
            lines.push(detail_line(
                "fields",
                "<none>",
                theme,
                DetailValueStyle::Muted,
            ));
        } else {
            for (key, value) in &self.fields {
                lines.push(detail_line(
                    key,
                    format_value(value),
                    theme,
                    DetailValueStyle::Data,
                ));
            }
        }

        for (index, span) in self.spans.iter().enumerate() {
            lines.extend(span_detail_lines(index, span, theme));
        }

        lines
    }
}

/// Semantic style used for a value in an expanded detail line.
#[derive(Debug, Clone, Copy)]
enum DetailValueStyle {
    Muted,
    Info,
    Accent,
    Data,
}

impl DetailValueStyle {
    /// Resolves this semantic value style through the supplied theme.
    fn resolve(self, theme: &Theme) -> Style {
        match self {
            Self::Muted => theme.muted,
            Self::Info => theme.info,
            Self::Accent => theme.accent,
            Self::Data => theme.data,
        }
    }
}

/// Builds an aligned key/value detail line.
fn detail_line(
    key: impl Into<String>,
    value: impl Into<String>,
    theme: &Theme,
    value_style: DetailValueStyle,
) -> Line<'static> {
    Line::from(vec![
        Span::raw(DETAIL_INDENT),
        Span::styled(format!("{:<14}", key.into()), theme.label),
        Span::styled(value.into(), value_style.resolve(theme)),
    ])
}

/// Builds the detail lines for one active tracing span.
fn span_detail_lines(index: usize, span: &LogSpan, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(1_usize.saturating_add(span.fields.len()));

    lines.push(detail_line(
        format!("span[{index}]"),
        span.name.clone(),
        theme,
        DetailValueStyle::Accent,
    ));

    for (key, value) in &span.fields {
        lines.push(detail_line(
            format!("  {key}"),
            format_value(value),
            theme,
            DetailValueStyle::Data,
        ));
    }

    lines
}

/// Formats a structured JSON value for terminal display.
fn format_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => "null".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_else(|error| {
                debug!(
                    %error,
                    "failed to format structured log value"
                );

                "<invalid JSON>".to_owned()
            })
        }
    }
}

/// Truncates text without splitting UTF-8 characters.
fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut characters = value.chars();

    let preview = characters.by_ref().take(max_chars).collect::<String>();

    if characters.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}
