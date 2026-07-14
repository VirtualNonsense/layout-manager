use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{List, ListItem, ListState},
};
use uuid::Uuid;

use crate::ui::{
    ComponentId, Direction2D,
    command::{ComponentCommand, PointerButton, PointerEvent, PointerGesture},
    component::{Component, ComponentKind, EventOutcome, RenderContext},
    input::PointerBinding,
};

pub struct SidebarComponent {
    id: ComponentId,
    items: Vec<String>,
    state: ListState,
}

impl Default for SidebarComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl SidebarComponent {
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            id: Uuid::new_v4(),
            items: vec!["Overview".into(), "Settings".into(), "Logs".into()],
            state,
        }
    }

    fn select_next(&mut self) {
        let selected = self.state.selected().unwrap_or(0);
        let next = (selected + 1).min(self.items.len().saturating_sub(1));
        self.state.select(Some(next));
    }

    fn select_previous(&mut self) {
        let selected = self.state.selected().unwrap_or(0);
        self.state.select(Some(selected.saturating_sub(1)));
    }

    fn click(&mut self, event: PointerEvent) {
        if let Some(local_y) = event.local_y {
            let index = local_y.saturating_sub(1) as usize;
            if index < self.items.len() {
                self.state.select(Some(index));
            }
        }
    }
}

impl Component for SidebarComponent {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn kind() -> ComponentKind {
        "sidebar"
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>) {
        let block = super::focused_block("Menu", ctx.focused);
        let items = self.items.iter().map(|item| ListItem::new(item.as_str()));
        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )
            .highlight_symbol("› ");
        frame.render_stateful_widget(list, area, &mut self.state);
    }

    fn on(&mut self, cmd: ComponentCommand) -> EventOutcome {
        match cmd {
            ComponentCommand::Move(Direction2D::Up) => self.select_previous(),
            ComponentCommand::Move(Direction2D::Down) => self.select_next(),
            ComponentCommand::Pointer(event) => self.click(event),
            _ => return EventOutcome::Ignored,
        }
        EventOutcome::Consumed(vec![])
    }

    fn key_bindings() -> &'static [(KeyCode, KeyModifiers, ComponentCommand)] {
        &[
            (
                KeyCode::Up,
                KeyModifiers::NONE,
                ComponentCommand::Move(Direction2D::Up),
            ),
            (
                KeyCode::Down,
                KeyModifiers::NONE,
                ComponentCommand::Move(Direction2D::Down),
            ),
        ]
    }

    fn pointer_bindings() -> &'static [(PointerGesture, PointerBinding)] {
        &[
            (
                PointerGesture::Down(PointerButton::Left),
                PointerBinding::WithEvent,
            ),
            (
                PointerGesture::ScrollUp,
                PointerBinding::Fixed(ComponentCommand::Move(Direction2D::Up)),
            ),
            (
                PointerGesture::ScrollDown,
                PointerBinding::Fixed(ComponentCommand::Move(Direction2D::Down)),
            ),
        ]
    }
}
