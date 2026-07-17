//! Type-erased storage for [`Component`] instances.

use crate::event::component::Event;
use crate::ui::UiAction;
use crate::ui::component::Component;
use crate::ui::component::context::{ComponentKind, EventOutcome, RenderContext};
use crate::ui::layout::ComponentId;
use ratatui::{Frame, layout::Rect};
use std::collections::HashMap;

/// Internal object-safe adapter used by [`ComponentRegistry`].
///
/// Kept private so application code always works with the typed [`Component`] trait.
pub trait ComponentAdapter {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>);
    fn on(&mut self, event: Box<dyn Event>) -> EventOutcome;
    fn get_kind(&self) -> ComponentKind;
}

impl<T: Component + 'static> ComponentAdapter for T {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>) {
        Component::render(self, frame, area, ctx);
    }

    fn on(&mut self, event: Box<dyn Event>) -> EventOutcome {
        Component::on(self, event)
    }

    fn get_kind(&self) -> ComponentKind {
        T::kind()
    }
}

/// Stores all mounted components, keyed by their [`ComponentId`].
///
/// Components are inserted as their concrete types and stored behind a
/// `Box<dyn ComponentAdapter>`, allowing heterogeneous collections without
/// trait objects on the public API.
#[derive(Default)]
pub struct ComponentRegistry {
    components: HashMap<ComponentId, Box<dyn ComponentAdapter>>,
}

impl ComponentRegistry {
    /// Insert a component.  Its ID is used as the map key.
    pub fn insert<C>(&mut self, component: C)
    where
        C: Component + 'static,
    {
        let id = component.id();
        self.components.insert(id, Box::new(component));
    }

    /// Return `true` if a component with `id` is registered.
    pub fn contains(&self, id: &ComponentId) -> bool {
        self.components.contains_key(id)
    }

    /// Iterate over all registered component IDs.
    pub fn ids(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.components.keys().copied()
    }

    /// Render the component with the given `id`, if present.
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

    /// Dispatch an event to the component with `id`.
    ///
    /// Returns [`EventOutcome::Ignored`] if no component with that ID is registered.
    pub fn on(&mut self, id: &ComponentId, event: Box<dyn Event>) -> EventOutcome {
        let opt_adapter = self.components.get_mut(id);
        match opt_adapter {
            Some(adapter) => adapter.on(event),
            None => EventOutcome::Ignored(event),
        }
    }

    /// Dispatch an event for all components.
    ///
    /// Returns [`EventOutcome`] for each component.
    pub fn on_broadcast_cloned(
        &mut self,
        event: Box<dyn Event>,
    ) -> impl Iterator<Item = EventOutcome> {
        let mut outcomes = vec![];
        for component in self.components_iter_mut() {
            outcomes.push(component.on(event.clone()));
        }
        outcomes.into_iter()
    }
    /// Dispatch an event for all components.
    ///
    /// Returns [`EventOutcome`] for each component.
    pub fn on_broadcast_till_consumed(&mut self, mut event: Box<dyn Event>) -> Vec<UiAction> {
        for component in self.components_iter_mut() {
            let event_name = event.event_name();
            match component.on(event) {
                EventOutcome::Ignored(return_event) => {
                    tracing::trace!("{} ignored in {}", event_name, component.get_kind());
                    event = return_event;
                    continue;
                }
                EventOutcome::Consumed(ui_actions) => {
                    tracing::trace!("{} consumed in {}", event_name, component.get_kind());
                    return ui_actions;
                }
            }
        }
        vec![]
    }

    /// Return the [`ComponentKind`] string for the component with `id`, if present.
    pub fn get_kind(&self, id: &ComponentId) -> Option<ComponentKind> {
        self.components.get(id).map(|c| c.get_kind())
    }

    fn components_iter_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn ComponentAdapter>> {
        self.components.values_mut()
    }
}
