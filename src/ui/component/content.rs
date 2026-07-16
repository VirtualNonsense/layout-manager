//! Content component — the main right-hand pane.

use std::fmt::Display;

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Stylize},
    widgets::Paragraph,
};
use tracing::info;
use uuid::Uuid;

use crate::ui::{
    ComponentId,
    command::{PointerEvent, PointerGesture},
    component::{
        Component, ComponentKind, EventOutcome, RenderContext,
        events::{MouseEvent, MoveEvent},
        widgets::focused_block,
    },
};
use crate::{
    event::component::Event,
    new_event,
    ui::widget::{WidgetList, WidgetListState},
};

#[derive(Debug, Clone)]
pub enum ContentMode {
    Counter,
    Help,
    Logs,
}
impl ContentMode {
    pub const fn all() -> &'static [ContentMode] {
        &[ContentMode::Counter, ContentMode::Help, ContentMode::Logs]
    }
    pub const fn as_str(&self) -> &'static str {
        match self {
            ContentMode::Counter => "Counter",
            ContentMode::Help => "Help",
            ContentMode::Logs => "Logs",
        }
    }
}

impl Display for ContentMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

new_event!(ContentComponentEvent, ContentMode);

/// Demonstration content pane.
///
/// Displays the current focus ID, an integer counter, and the last click
/// coordinates.  Handles [`MoveEvent`] (keyboard up/down increments/decrements
/// the counter) and [`MouseEvent`] (scroll does the same).
pub struct ContentComponent {
    id: ComponentId,
    counter: i64,
    last_click: Option<PointerEvent>,
    state: ContentMode,
    option_log_list_state: Option<WidgetListState>,
}

impl Default for ContentComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentComponent {
    /// Create a content component with the counter at zero.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            counter: 0,
            last_click: None,
            state: ContentMode::Logs,
            option_log_list_state: None,
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

        match self.state {
            ContentMode::Counter => {
                let text = format!("Counter: {}", self.counter);
                let paragraph = Paragraph::new(text)
                    .block(focused_block("Counter View", ctx.focused))
                    .alignment(Alignment::Center)
                    .fg(Color::Cyan)
                    .bg(Color::Black);
                frame.render_widget(paragraph, area);
            }
            ContentMode::Help => {
                let text = format!(
                    "Associated command types + key/mouse input:\n\n\
                     q / Esc / Ctrl-C   Quit through original AppEvent channel\n\
                     Tab / Shift-Tab    Fokus wechseln\n\
                     Alt+Pfeile         Geometrische Fokus-Navigation\n\
                     Menu: ↑/↓ + click  Stateful ListState\n\
                     Content: ←/→       Counter ändern\n\
                     Content: click     Store click coordinates\n\n\
                     Focus: {}\n\
                     click_text: {}\n\
                     mode: {}",
                    ctx.focus_id, click_text, self.state,
                );
                let paragraph = Paragraph::new(text)
                    .block(focused_block("Help View", ctx.focused))
                    .alignment(Alignment::Center)
                    .fg(Color::Cyan)
                    .bg(Color::Black);
                frame.render_widget(paragraph, area);
            }
            ContentMode::Logs => {
                let entries = 40;
                let mut list_state = self.option_log_list_state.clone().unwrap_or_default();

                let iter = crate::log::tail_log_entries(entries).expect("failed to unpack logs");
                let block = focused_block("Log view", ctx.focused);
                frame.render_stateful_widget(
                    WidgetList::new(iter.collect(), 2).block(Some(block)),
                    area,
                    &mut list_state,
                );
                self.option_log_list_state = Some(list_state);
            }
        };
    }

    fn on(&mut self, event: &dyn Event) -> EventOutcome {
        let delta = if let Some(MoveEvent(direction)) = event.downcast_ref::<MoveEvent>() {
            match direction {
                crate::ui::Direction2D::Right => 1,
                crate::ui::Direction2D::Left => -1,
                crate::ui::Direction2D::Up | crate::ui::Direction2D::Down => {
                    return EventOutcome::Ignored;
                }
            }
        } else if let Some(MouseEvent(pointer)) = event.downcast_ref::<MouseEvent>() {
            match pointer.gesture {
                PointerGesture::ScrollUp => -1,
                PointerGesture::ScrollDown => 1,
                _ => return EventOutcome::Ignored,
            }
        } else {
            0
        };
        match self.state {
            ContentMode::Counter => self.counter = self.counter.saturating_add(delta),
            ContentMode::Help => {}
            ContentMode::Logs => {
                let mut state = self.option_log_list_state.clone().unwrap_or_default();
                let selected = state
                    .selected()
                    .unwrap_or_default()
                    .saturating_sub_signed(delta as isize);
                info!("scroll state set to {selected}");
                state.select(Some(selected));
                self.option_log_list_state = Some(state);
            }
        }

        if let Some(ContentComponentEvent(content_mode)) =
            event.downcast_ref::<ContentComponentEvent>()
        {
            self.state = content_mode.clone()
        }
        EventOutcome::Consumed(vec![])
    }
}
