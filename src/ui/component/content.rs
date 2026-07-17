//! Content component — the main right-hand pane.

use std::fmt::Display;

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Stylize},
    widgets::Paragraph,
};
use uuid::Uuid;

use crate::{
    event::component::Event,
    new_event,
    ui::{
        UiAction,
        widget::{WidgetList, WidgetListState},
    },
};
use crate::{
    log::LogEntry,
    ui::{
        ComponentId, Direction2D,
        command::{PointerEvent, PointerGesture},
        component::{
            Component, ComponentKind, EventOutcome, RenderContext,
            events::{MouseEvent, MoveEvent},
            widgets::focused_block,
        },
    },
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

new_event!(
    enum ContentComponentEvent {
        ContentMode(ContentMode),
        NewLogs(Vec<crate::log::LogEntry>),
    }
);

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
    log_entries: usize,
    current_logs: Option<Vec<LogEntry>>,
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
            log_entries: 400,
            current_logs: None,
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
                let block = focused_block("Log view", ctx.focused);
                match &self.current_logs {
                    Some(entries) => {
                        let mut list_state = self.option_log_list_state.take().unwrap_or_default();

                        frame.render_stateful_widget(
                            WidgetList::new(entries, 1).block(Some(block)),
                            area,
                            &mut list_state,
                        );
                        self.option_log_list_state = Some(list_state);
                    }
                    None => frame
                        .render_widget(Paragraph::new("waiting for logging").block(block), area),
                }
            }
        };
    }

    fn on(&mut self, event: Box<dyn Event>) -> EventOutcome {
        let delta = if let Some(MoveEvent(direction)) = event.downcast_ref::<MoveEvent>() {
            match direction {
                Direction2D::Right | Direction2D::Up => 1,
                Direction2D::Left | Direction2D::Down => -1,
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
                let mut state: WidgetListState =
                    self.option_log_list_state.clone().unwrap_or_default();
                if delta <= 0 {
                    state.select_previous(self.log_entries);
                } else {
                    state.select_next(self.log_entries);
                }
                self.option_log_list_state = Some(state);
            }
        }

        if let Some(ContentComponentEvent::ContentMode(content_mode)) =
            event.downcast_ref::<ContentComponentEvent>()
        {
            self.state = content_mode.clone()
        }
        if let Some(ContentComponentEvent::NewLogs(logs)) = event.downcast_to() {
            self.current_logs = Some(logs);
        }
        EventOutcome::Consumed(vec![UiAction::App(crate::ui::AppCommand::FetchLogs {
            origin: self.id,
            amount: self.log_entries,
        })])
    }
}
