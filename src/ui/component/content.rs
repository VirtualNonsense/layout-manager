//! Content component: the main right-hand pane.

use ratatui::{Frame, layout::Rect};
use tracing::instrument;
use tracing::warn;
use uuid::Uuid;

use crate::event::Event;
use crate::event::Tick;
use crate::ui::component::widgets::focused_block;
use crate::ui_lib::component::{
    Component, ComponentId, ComponentKind, EventOutcome, RenderContext,
};
use crate::ui_lib::events::Submit;

pub struct MainView {
    id: ComponentId,
}

impl Default for MainView {
    fn default() -> Self {
        Self::new()
    }
}

impl MainView {
    /// Creates the main content component.
    pub fn new() -> Self {
        Self { id: Uuid::new_v4() }
    }

    /// Handles a periodic tick.
    fn handle_tick(&mut self, tick: Tick) -> EventOutcome {
        EventOutcome::Ignored(tick.boxed())
    }
}

impl Component for MainView {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn kind() -> ComponentKind {
        "MainView"
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, context: RenderContext<'_>) {
        let block = focused_block("Content", context.focused);

        frame.render_widget(block, area);
    }

    #[instrument(skip(self))]
    fn on(&mut self, mut event: Box<dyn Event>) -> EventOutcome {
        event = match event.downcast_to::<Tick>() {
            Ok(tick) => {
                return self.handle_tick(tick);
            }
            Err(event) => event,
        };

        event = match event.downcast_to::<Submit>() {
            Ok(submit) => submit.boxed(),
            Err(event) => event,
        };

        EventOutcome::Ignored(event)
    }
}
