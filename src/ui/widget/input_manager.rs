use crate::ui_lib::component::ComponentKind;
use crate::ui_lib::input::{AvailableKeyBinding, BindingScope, InputManager, KeyStroke};
use crate::ui_lib::theme::Theme;
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    text::{Line, Span},
    widgets::{Block, Cell, Row, StatefulWidget, Table, TableState, Widget},
};
use std::mem;
use std::time::{Duration, Instant};

const DEFAULT_SCROLL_INTERVAL: Duration = Duration::from_millis(250);
const DEFAULT_SCROLL_PAUSE: Duration = Duration::from_millis(1_000);
const DEFAULT_SCROLL_GAP: usize = 3;
const COLUMN_SPACING: u16 = 1;

pub struct InputManagerWidget<'a> {
    input_manager: &'a InputManager,
    theme: &'a Theme,
    block: Option<Block<'a>>,
    show_scope: bool,
    scroll_interval: Duration,
    scroll_pause: Duration,
    scroll_gap: usize,
}

impl<'a> InputManagerWidget<'a> {
    #[must_use]
    pub fn new(input_manager: &'a InputManager, theme: &'a Theme) -> Self {
        Self {
            input_manager,
            theme,
            block: Some(theme.block("Keyboard shortcuts")),
            show_scope: false,
            scroll_interval: DEFAULT_SCROLL_INTERVAL,
            scroll_pause: DEFAULT_SCROLL_PAUSE,
            scroll_gap: DEFAULT_SCROLL_GAP,
        }
    }

    #[must_use]
    pub fn theme(mut self, theme: &'a Theme) -> Self {
        self.block = Some(theme.block("Keyboard shortcuts"));
        self.theme = theme;
        self
    }

    #[must_use]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    #[must_use]
    pub fn without_block(mut self) -> Self {
        self.block = None;
        self
    }

    #[must_use]
    pub fn show_scope(mut self, show_scope: bool) -> Self {
        self.show_scope = show_scope;
        self
    }
    #[must_use]
    pub fn scroll_pause(mut self, scroll_pause: Duration) -> Self {
        self.scroll_pause = scroll_pause;
        self
    }

    #[must_use]
    pub fn scroll_interval(mut self, scroll_interval: Duration) -> Self {
        self.scroll_interval = scroll_interval;
        self
    }

    #[must_use]
    pub fn scroll_gap(mut self, scroll_gap: usize) -> Self {
        self.scroll_gap = scroll_gap;
        self
    }

    fn binding_row(
        &self,
        binding: &AvailableKeyBinding,
        widths: &[u16],
        offsets: &[usize],
        pause_steps: usize,
    ) -> Row<'static> {
        let key = format_key_stroke(binding.key);

        let key = Cell::from(scroll_cell(
            &key,
            usize::from(widths[0]),
            offsets[0],
            self.scroll_gap,
            pause_steps,
        ))
        .style(self.theme.accent);

        let description = Cell::from(scroll_cell(
            binding.description,
            usize::from(widths[1]),
            offsets[1],
            self.scroll_gap,
            pause_steps,
        ))
        .style(self.theme.text);

        if self.show_scope {
            let scope = Cell::from(scroll_cell(
                format_scope(binding.scope),
                usize::from(widths[2]),
                offsets[2],
                self.scroll_gap,
                pause_steps,
            ))
            .style(scope_style(self.theme, binding.scope));

            Row::new([key, description, scope])
        } else {
            Row::new([key, description])
        }
    }
}

#[derive(Debug)]
pub struct InputManagerWidgetState {
    focused: Option<ComponentKind>,
    table: TableState,
    cell_offsets: Vec<Vec<usize>>,
    last_scroll_update: Instant,
}

impl Default for InputManagerWidgetState {
    fn default() -> Self {
        Self {
            focused: None,
            table: TableState::default(),
            cell_offsets: Vec::new(),
            last_scroll_update: Instant::now(),
        }
    }
}

impl InputManagerWidgetState {
    #[must_use]
    pub fn new(focused: Option<ComponentKind>) -> Self {
        Self {
            focused,
            table: TableState::default(),
            cell_offsets: Vec::new(),
            last_scroll_update: Instant::now(),
        }
    }

    #[must_use]
    pub fn focused(&self) -> Option<ComponentKind> {
        self.focused
    }

    pub fn set_focused(&mut self, focused: Option<ComponentKind>) {
        if self.focused == focused {
            return;
        }

        self.focused = focused;
        self.table.select(None);
        self.set_table_offset(0);
        self.reset_cell_scrolling();
    }

    pub fn scroll_up(&mut self) {
        let offset = self.table.offset().saturating_sub(1);
        self.set_table_offset(offset);
    }

    pub fn scroll_down(&mut self) {
        let offset = self.table.offset().saturating_add(1);
        self.set_table_offset(offset);
    }

    fn set_table_offset(&mut self, offset: usize) {
        self.table = mem::take(&mut self.table).with_offset(offset);
    }

    fn reset_cell_scrolling(&mut self) {
        self.cell_offsets.clear();
        self.last_scroll_update = Instant::now();
    }

    fn prepare_cell_offsets(&mut self, rows: usize, columns: usize) {
        self.cell_offsets.resize_with(rows, || vec![0; columns]);

        for offsets in &mut self.cell_offsets {
            offsets.resize(columns, 0);
        }
    }

    fn update_cell_offsets(&mut self, interval: Duration) {
        if interval.is_zero() {
            return;
        }

        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last_scroll_update);
        let interval_nanos = interval.as_nanos();
        let elapsed_steps = elapsed.as_nanos() / interval_nanos;

        if elapsed_steps == 0 {
            return;
        }

        let steps = usize::try_from(elapsed_steps).unwrap_or(usize::MAX);

        for row_offsets in &mut self.cell_offsets {
            for offset in row_offsets {
                *offset = offset.saturating_add(steps);
            }
        }

        self.last_scroll_update = now;
    }
}

impl StatefulWidget for InputManagerWidget<'_> {
    type State = InputManagerWidgetState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let inner = if let Some(block) = &self.block {
            let inner = block.inner(area);
            block.render(area, buffer);
            inner
        } else {
            area
        };

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let bindings = self.input_manager.available_key_bindings(state.focused);

        if bindings.is_empty() {
            state.cell_offsets.clear();

            Line::from(Span::styled(
                "No keyboard shortcuts available",
                self.theme.muted,
            ))
            .render(inner, buffer);

            return;
        }

        let column_count = if self.show_scope { 3 } else { 2 };
        let widths = resolved_column_widths(inner.width, self.show_scope);

        state.prepare_cell_offsets(bindings.len(), column_count);
        state.update_cell_offsets(self.scroll_interval);

        let pause_steps = duration_to_steps(self.scroll_pause, self.scroll_interval);

        let rows = bindings.iter().enumerate().map(|(index, binding)| {
            self.binding_row(binding, &widths, &state.cell_offsets[index], pause_steps)
        });

        let constraints = widths
            .iter()
            .copied()
            .map(Constraint::Length)
            .collect::<Vec<_>>();

        let header = if self.show_scope {
            Row::new([Cell::from("Key"), Cell::from("Action"), Cell::from("Scope")])
        } else {
            Row::new([Cell::from("Key"), Cell::from("Action")])
        }
        .style(self.theme.title)
        .bottom_margin(1);

        let table = Table::new(rows, constraints)
            .header(header)
            .column_spacing(COLUMN_SPACING)
            .row_highlight_style(self.theme.selected);

        StatefulWidget::render(table, inner, buffer, &mut state.table);
    }
}

fn resolved_column_widths(total_width: u16, show_scope: bool) -> Vec<u16> {
    if show_scope {
        resolve_widths(total_width, &[18, 11], 3)
    } else {
        resolve_widths(total_width, &[13], 2)
    }
}

fn resolve_widths(total_width: u16, fixed_widths: &[u16], column_count: usize) -> Vec<u16> {
    let spacing_count = column_count.saturating_sub(1);
    let spacing_count = u16::try_from(spacing_count).unwrap_or(u16::MAX);
    let spacing = COLUMN_SPACING.saturating_mul(spacing_count);

    let available_width = total_width.saturating_sub(spacing);
    let fixed_total = fixed_widths
        .iter()
        .copied()
        .fold(0_u16, u16::saturating_add);

    if fixed_total < available_width {
        let mut widths = fixed_widths.to_vec();
        widths.push(available_width - fixed_total);
        return widths;
    }

    let column_count = u16::try_from(column_count).unwrap_or(1);
    let base_width = available_width / column_count;
    let remainder = available_width % column_count;

    (0..column_count)
        .map(|index| base_width + u16::from(index < remainder))
        .collect()
}

fn duration_to_steps(duration: Duration, interval: Duration) -> usize {
    if duration.is_zero() || interval.is_zero() {
        return 0;
    }

    let duration_nanos = duration.as_nanos();
    let interval_nanos = interval.as_nanos();

    let steps = duration_nanos.saturating_add(interval_nanos.saturating_sub(1)) / interval_nanos;

    usize::try_from(steps).unwrap_or(usize::MAX)
}

fn scroll_cell(text: &str, width: usize, offset: usize, gap: usize, pause_steps: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let characters = text.chars().collect::<Vec<_>>();

    if characters.len() <= width {
        return text.to_owned();
    }

    let gap = gap.max(1);
    let scrolling_steps = characters.len().saturating_add(gap);
    let cycle_steps = pause_steps.saturating_add(scrolling_steps);

    if cycle_steps == 0 {
        return characters.into_iter().take(width).collect();
    }

    let cycle_position = offset % cycle_steps;

    let start = if cycle_position < pause_steps {
        0
    } else {
        cycle_position.saturating_sub(pause_steps).saturating_add(1) % scrolling_steps
    };

    (0..width)
        .map(|index| {
            let position = (start + index) % scrolling_steps;

            if position < characters.len() {
                characters[position]
            } else {
                ' '
            }
        })
        .collect()
}

fn scope_style(theme: &Theme, scope: BindingScope) -> ratatui::style::Style {
    match scope {
        BindingScope::Component => theme.info,
        BindingScope::Application => theme.muted,
    }
}

fn format_scope(scope: BindingScope) -> &'static str {
    match scope {
        BindingScope::Component => "Component",
        BindingScope::Application => "Global",
    }
}

fn format_key_stroke(key: KeyStroke) -> String {
    let mut parts = Vec::new();

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl".to_owned());
    }

    if key.modifiers.contains(KeyModifiers::ALT) {
        parts.push("Alt".to_owned());
    }

    if key.modifiers.contains(KeyModifiers::SHIFT) && !matches!(key.code, KeyCode::BackTab) {
        parts.push("Shift".to_owned());
    }

    if key.modifiers.contains(KeyModifiers::SUPER) {
        parts.push("Super".to_owned());
    }

    if key.modifiers.contains(KeyModifiers::HYPER) {
        parts.push("Hyper".to_owned());
    }

    if key.modifiers.contains(KeyModifiers::META) {
        parts.push("Meta".to_owned());
    }

    parts.push(format_key_code(key.code));

    let mut formatted = parts.join("+");

    match key.kind {
        KeyEventKind::Press => {}
        KeyEventKind::Repeat => formatted.push_str(" (hold)"),
        KeyEventKind::Release => formatted.push_str(" (release)"),
    }

    formatted
}

fn format_key_code(code: KeyCode) -> String {
    match code {
        KeyCode::Backspace => "Back".to_owned(),
        KeyCode::Enter => "Enter".to_owned(),
        KeyCode::Left => "←".to_owned(),
        KeyCode::Right => "→".to_owned(),
        KeyCode::Up => "↑".to_owned(),
        KeyCode::Down => "↓".to_owned(),
        KeyCode::Home => "Home".to_owned(),
        KeyCode::End => "End".to_owned(),
        KeyCode::PageUp => "Page Up".to_owned(),
        KeyCode::PageDown => "Page Down".to_owned(),
        KeyCode::Tab => "Tab".to_owned(),
        KeyCode::BackTab => "Shift+Tab".to_owned(),
        KeyCode::Delete => "Delete".to_owned(),
        KeyCode::Insert => "Insert".to_owned(),
        KeyCode::F(number) => format!("F{number}"),
        KeyCode::Char(character) => character.to_uppercase().collect(),
        KeyCode::Null => "Null".to_owned(),
        KeyCode::Esc => "Esc".to_owned(),
        KeyCode::CapsLock => "Caps Lock".to_owned(),
        KeyCode::ScrollLock => "Scroll Lock".to_owned(),
        KeyCode::NumLock => "Num Lock".to_owned(),
        KeyCode::PrintScreen => "Print Screen".to_owned(),
        KeyCode::Pause => "Pause".to_owned(),
        KeyCode::Menu => "Menu".to_owned(),
        KeyCode::KeypadBegin => "Keypad Begin".to_owned(),
        KeyCode::Media(media_key) => format!("{media_key:?}"),
        KeyCode::Modifier(modifier_key) => format!("{modifier_key:?}"),
    }
}
