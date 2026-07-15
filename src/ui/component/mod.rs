use crate::ui::command::UiAction;
use crate::ui::component::component_event::Event;
use crate::ui::layout::ComponentId;
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{Block, BorderType},
};
use std::collections::HashMap;

pub mod component_event;
pub mod content;
pub mod sidebar;
pub use content::ContentComponent;
pub use sidebar::SidebarComponent;

pub struct RenderContext<'a> {
    pub focused: bool,
    pub focus_id: &'a ComponentId,
}

pub enum EventOutcome {
    Ignored,
    Consumed(Vec<UiAction>),
}

pub type ComponentKind = &'static str;

/// Strongly typed, self-describing component trait.
///
/// Each component defines its own `ComponentCommand`-to-action mapping in `on()` and declares its own
/// keybindings via `key_bindings()` / `pointer_bindings()`. No central command enum exists —
/// adding a new component requires only this file; nothing else needs to be touched.
pub trait Component {
    fn id(&self) -> ComponentId;
    fn kind() -> ComponentKind
    where
        Self: Sized;
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>);

    /// Handle an abstract `ComponentCommand`. Components interpret only the `ComponentCommand` variants they understand
    /// and return `EventOutcome::Ignored` for everything else.
    fn on(&mut self, event: &dyn Event) -> EventOutcome;
}

/// Internal object-safe adapter used by the registry.
///
/// This stays private so app code works with the typed [`Component`] trait.
/// The adapter receives the same abstract `ComponentCommand` that was produced by the input layer —
/// no downcasting or type-erased Any needed: `ComponentCommand` itself is the lingua franca.
trait ComponentAdapter {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>);
    fn on(&mut self, cmd: &dyn Event) -> EventOutcome;
    fn get_kind(&self) -> ComponentKind;
}

impl<T: Component + 'static> ComponentAdapter for T {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>) {
        Component::render(self, frame, area, ctx);
    }

    fn on(&mut self, event: &dyn Event) -> EventOutcome {
        Component::on(self, event)
    }

    fn get_kind(&self) -> ComponentKind {
        T::kind()
    }
}

#[derive(Default)]
pub struct ComponentRegistry {
    components: HashMap<ComponentId, Box<dyn ComponentAdapter>>,
}

impl ComponentRegistry {
    pub fn insert<C>(&mut self, component: C)
    where
        C: Component + 'static,
    {
        let id = component.id();
        self.components.insert(id, Box::new(component));
    }

    pub fn contains(&self, id: &ComponentId) -> bool {
        self.components.contains_key(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.components.keys().copied()
    }

    pub fn render(
        &mut self,
        id: &ComponentId,
        frame: &mut Frame,
        area: Rect,
        ctx: RenderContext<'_>,
    ) {
        if let Some(component) = self.components.get_mut(id) {
            component.render(frame, area, ctx);
        }
    }

    pub fn on(&mut self, id: &ComponentId, event: &dyn Event) -> EventOutcome {
        self.components
            .get_mut(id)
            .map(|component| component.on(event))
            .unwrap_or(EventOutcome::Ignored)
    }

    pub fn get_kind(&self, id: &ComponentId) -> Option<ComponentKind> {
        self.components.get(id).map(|c| c.get_kind())
    }
}

// ─── Shared helpers ──────────────────────────────────────────────────────────

fn focused_block(title: &'static str, focused: bool) -> Block<'static> {
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Block::bordered()
        .title(title)
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
}
