//! Type-erased storage for [`Component`] instances.

use crate::event::Event;
use crate::ui_lib::component::{
    Component, ComponentId, ComponentKind, EventOutcome, RenderContext,
};

use ratatui::{Frame, layout::Rect};
use std::collections::HashMap;
use tracing::{instrument, trace};

/// Internal object-safe adapter used by [`ComponentRegistry`].
///
/// Kept private so application code works with the typed [`Component`] trait.
trait ComponentAdapter {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>);

    fn on(&mut self, event: Box<dyn Event>) -> EventOutcome;

    fn get_kind(&self) -> ComponentKind;
}

impl<T> ComponentAdapter for T
where
    T: Component + 'static,
{
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>) {
        Component::render(self, frame, area, ctx)
    }

    fn on(&mut self, event: Box<dyn Event>) -> EventOutcome {
        Component::on(self, event)
    }

    fn get_kind(&self) -> ComponentKind {
        T::kind()
    }
}

/// Stores mounted components keyed by [`ComponentId`].
#[derive(Default)]
pub struct ComponentRegistry {
    // does not probide a stable ordering: Vec<(ComponentId, Box<dyn ComponentAdapter>)>
    components: HashMap<ComponentId, Box<dyn ComponentAdapter>>,
}

impl ComponentRegistry {
    /// Inserts a component into the registry.
    pub fn insert<C>(&mut self, component: C)
    where
        C: Component + 'static,
    {
        let id = component.id();
        self.components.insert(id, Box::new(component));
    }

    /// Returns whether a component with the given ID is registered.
    pub fn contains(&self, id: &ComponentId) -> bool {
        self.components.contains_key(id)
    }

    /// Iterates over all registered component IDs.
    pub fn ids(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.components.keys().copied()
    }

    /// Renders the component with the given ID.
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

    /// Dispatches an owned event to one component.
    ///
    /// If no component with that ID exists, the event is returned as ignored.
    #[instrument(skip(self))]
    pub fn on(&mut self, id: &ComponentId, event: Box<dyn Event>) -> EventOutcome {
        match self.components.get_mut(id) {
            Some(component) => component.on(event),
            None => EventOutcome::Ignored(event),
        }
    }

    /// Clones and broadcasts an event to every component.
    ///
    /// This should only be used for events whose payload is intentionally
    /// cloneable. Large ownership-heavy events should use
    /// [`Self::on_broadcast_till_consumed`].
    pub fn on_broadcast_cloned(&mut self, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        let event_name = event.event_name();
        let mut reactions = Vec::new();

        for component in self.components.values_mut() {
            let kind = component.get_kind();

            match component.on(event.clone()) {
                EventOutcome::Ignored(_) => {
                    trace!(
                        event = event_name,
                        component = kind,
                        "component ignored cloned broadcast event"
                    );
                }

                EventOutcome::Consumed(mut component_reactions) => {
                    trace!(
                        event = event_name,
                        component = kind,
                        reaction_count = component_reactions.len(),
                        "component consumed cloned broadcast event"
                    );

                    reactions.append(&mut component_reactions);
                }
            }
        }

        reactions
    }

    /// Offers an owned event to components until one consumes it.
    ///
    /// No event cloning occurs. If no component consumes the event, ownership
    /// is returned through [`EventOutcome::Ignored`].
    pub fn on_broadcast_till_consumed(&mut self, mut event: Box<dyn Event>) -> EventOutcome {
        let event_name = event.event_name();

        for component in self.components.values_mut() {
            let kind = component.get_kind();

            match component.on(event) {
                EventOutcome::Ignored(returned_event) => {
                    trace!(
                        event = event_name,
                        component = kind,
                        "component ignored event"
                    );

                    event = returned_event;
                }

                EventOutcome::Consumed(reactions) => {
                    trace!(
                        event = event_name,
                        component = kind,
                        reaction_count = reactions.len(),
                        "component consumed event"
                    );

                    return EventOutcome::Consumed(reactions);
                }
            }
        }

        trace!(event = event_name, "no component consumed event");

        EventOutcome::Ignored(event)
    }

    /// Returns the component kind for the given component ID.
    pub fn get_kind(&self, id: &ComponentId) -> Option<ComponentKind> {
        self.components
            .get(id)
            .map(|component| component.get_kind())
    }
}
