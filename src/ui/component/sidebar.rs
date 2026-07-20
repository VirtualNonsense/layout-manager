//! Sidebar component — a navigable list of items.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{List, ListItem, ListState},
};
use tracing::trace;
use uuid::Uuid;

use crate::ui::{
    ComponentId, UiAction,
    command::{PointerEvent, PointerGesture},
    component::{
        Component, ComponentKind, EventOutcome, RenderContext,
        content::ContentComponentEvent,
        events::{MouseEvent, MoveEvent, Submit},
        widgets::focused_block,
    },
};
use crate::{event::component::Event, ui::component::content::ContentMode};

/// A stateful list displayed in the left pane.
///
/// Handles [`MoveEvent`] (keyboard up/down) and [`MouseEvent`] (scroll and
/// click-to-select).  The selected item is highlighted with a yellow `›` symbol.
pub struct SidebarComponent {
    id: ComponentId,
    items: Vec<ContentMode>,
    state: ListState,
}

impl Default for SidebarComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl SidebarComponent {
    /// Create a sidebar pre-populated with the demo item list.
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            id: Uuid::new_v4(),
            items: ContentMode::all().to_vec(),
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

    /// simple click handler.
    /// will return true if an items was hit.
    fn click(&mut self, event: &PointerEvent) -> bool {
        if let Some(local_y) = event.local_y {
            let index = local_y.saturating_sub(1) as usize;
            if index < self.items.len() {
                self.state.select(Some(index));
                return true;
            }
        }
        false
    }

    fn get_selected(&self) -> ContentMode {
        self.items
            .get(self.state.selected().unwrap_or(0))
            .expect("Should not be empty")
            .clone()
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
        let block = focused_block("Menu", ctx.focused);
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

    fn on(&mut self, event: Box<dyn Event>) -> EventOutcome {
        if let Some(MoveEvent(direction)) = event.downcast_ref::<MoveEvent>() {
            match direction {
                crate::ui::Direction2D::Up => self.select_previous(),
                crate::ui::Direction2D::Down => self.select_next(),
                crate::ui::Direction2D::Left | crate::ui::Direction2D::Right => {
                    return EventOutcome::Ignored(event);
                }
            }
        }

        if let Some(MouseEvent(pointer)) = event.downcast_ref::<MouseEvent>() {
            trace!("pointer event: {:?}", pointer);
            match pointer.gesture {
                PointerGesture::ScrollUp => self.select_previous(),
                PointerGesture::ScrollDown => self.select_next(),
                PointerGesture::Down(_) => {
                    if self.click(pointer) {
                        let selected = self.get_selected();
                        return EventOutcome::Consumed(vec![UiAction::Response(Box::new(
                            ContentComponentEvent::ContentMode(selected),
                        ))]);
                    }
                }

                _ => return EventOutcome::Ignored(event),
            }
        }

        if event.downcast_ref::<Submit>().is_some() {
            let selected = self.get_selected();
            trace!("submit in sidebar!");
            return EventOutcome::Consumed(vec![UiAction::Response(Box::new(
                ContentComponentEvent::ContentMode(selected),
            ))]);
        }
        EventOutcome::Ignored(event)
    }
}
