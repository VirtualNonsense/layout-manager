use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{List, ListItem, ListState},
};
use uuid::Uuid;

use crate::ui::{
    ComponentId,
    command::{PointerEvent, PointerGesture},
    component::{
        Component, ComponentKind, EventOutcome, RenderContext,
        component_event::{Event, MouseEvent, MoveEvent},
    },
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

    fn click(&mut self, event: &PointerEvent) {
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

    fn on(&mut self, event: &dyn Event) -> EventOutcome {
        if let Some(MoveEvent(direction)) = event.downcast_ref::<MoveEvent>() {
            match direction {
                crate::ui::Direction2D::Up => self.select_previous(),
                crate::ui::Direction2D::Down => self.select_next(),
                crate::ui::Direction2D::Left | crate::ui::Direction2D::Right => {
                    return EventOutcome::Ignored;
                }
            }
        }

        if let Some(MouseEvent(pointer)) = event.downcast_ref::<MouseEvent>() {
            match pointer.gesture {
                PointerGesture::ScrollUp => self.select_previous(),
                PointerGesture::ScrollDown => self.select_next(),
                PointerGesture::Down(_) => self.click(pointer),

                _ => return EventOutcome::Ignored,
            }
        }
        EventOutcome::Consumed(vec![])
    }
}
