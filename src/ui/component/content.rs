use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Stylize},
    widgets::Paragraph,
};
use uuid::Uuid;

use crate::ui::{
    ComponentId,
    command::{ComponentCommand, PointerButton, PointerEvent, PointerGesture},
    component::{Component, ComponentKind, EventOutcome, RenderContext},
    input::PointerBinding,
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

    fn on(&mut self, cmd: ComponentCommand) -> EventOutcome {
        match cmd {
            ComponentCommand::Increment => self.counter += 1,
            ComponentCommand::Decrement => self.counter -= 1,
            ComponentCommand::Pointer(event) => {
                match event.gesture {
                    PointerGesture::ScrollUp => self.counter += 1,
                    PointerGesture::ScrollDown => self.counter -= 1,
                    _ => {}
                }
                self.last_click = Some(event);
            }
            _ => return EventOutcome::Ignored,
        }
        EventOutcome::Consumed(vec![])
    }

    fn key_bindings() -> &'static [(KeyCode, KeyModifiers, ComponentCommand)] {
        &[
            (
                KeyCode::Left,
                KeyModifiers::NONE,
                ComponentCommand::Decrement,
            ),
            (
                KeyCode::Right,
                KeyModifiers::NONE,
                ComponentCommand::Increment,
            ),
        ]
    }

    fn pointer_bindings() -> &'static [(PointerGesture, PointerBinding)] {
        &[
            (
                PointerGesture::Down(PointerButton::Left),
                PointerBinding::WithEvent,
            ),
            (PointerGesture::ScrollUp, PointerBinding::WithEvent),
            (PointerGesture::ScrollDown, PointerBinding::WithEvent),
        ]
    }
}
