use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Stylize},
    widgets::Paragraph,
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

pub struct ContentComponent {
    id: ComponentId,
    counter: i64,
    last_click: Option<PointerEvent>,
}

impl Default for ContentComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentComponent {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            counter: 0,
            last_click: None,
        }
    }
}

impl Component for ContentComponent {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn kind() -> ComponentKind {
        "content"
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>) {
        let click_text = self
            .last_click
            .map(|event| {
                format!(
                    "Last click: global=({}, {}), local=({:?}, {:?})",
                    event.x, event.y, event.local_x, event.local_y
                )
            })
            .unwrap_or_else(|| "Last click: none".to_owned());

        let text = format!(
            "Associated command types + key/mouse input:\n\n\
             q / Esc / Ctrl-C   Quit through original AppEvent channel\n\
             Tab / Shift-Tab    Fokus wechseln\n\
             Alt+Pfeile         Geometrische Fokus-Navigation\n\
             Menu: ↑/↓ + click  Stateful ListState\n\
             Content: ←/→       Counter ändern\n\
             Content: click     Store click coordinates\n\n\
             Focus: {}\n\
             Counter: {}\n\
             {}",
            ctx.focus_id, self.counter, click_text,
        );

        let paragraph = Paragraph::new(text)
            .block(super::focused_block("Content", ctx.focused))
            .alignment(Alignment::Center)
            .fg(Color::Cyan)
            .bg(Color::Black);
        frame.render_widget(paragraph, area);
    }

    fn on(&mut self, event: &dyn Event) -> EventOutcome {
        if let Some(MoveEvent(direction)) = event.downcast_ref::<MoveEvent>() {
            match direction {
                crate::ui::Direction2D::Up => self.counter += 1,
                crate::ui::Direction2D::Down => self.counter -= 1,
                crate::ui::Direction2D::Left | crate::ui::Direction2D::Right => {
                    return EventOutcome::Ignored;
                }
            }
        }

        if let Some(MouseEvent(pointer)) = event.downcast_ref::<MouseEvent>() {
            match pointer.gesture {
                PointerGesture::ScrollUp => self.counter += 1,
                PointerGesture::ScrollDown => self.counter -= 1,
                _ => return EventOutcome::Ignored,
            }
        }
        EventOutcome::Consumed(vec![])
    }
}
