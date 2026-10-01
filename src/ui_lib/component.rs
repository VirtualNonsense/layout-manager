use crate::{event::Event, ui_lib::input::InputManager};
use ratatui::{Frame, layout::Rect};
use uuid::Uuid;

/// Unique identifier for a component instance or a focus slot.
///
/// A [`Uuid`] is used so IDs can be generated at component construction time
/// without requiring a central registry.
pub type ComponentId = Uuid;

/// Type used to identify a component kind for input routing.
pub type ComponentKind = &'static str;

/// Data passed to a component's [`Component::render`] method.
#[derive(Debug)]
pub struct RenderContext<'a> {
    /// Whether this component currently holds keyboard focus.
    pub focused: bool,

    /// Focus-region ID associated with this render slot.
    pub focus_id: &'a ComponentId,

    pub focused_kind: Option<ComponentKind>,

    pub manager: &'a InputManager,
}

/// Result returned by a component's [`Component::on`] handler.
///
/// This outcome deliberately owns the event. A component that does not
/// understand an event must return it through [`EventOutcome::Ignored`].
///
/// This allows the same event to move through multiple components without
/// cloning. Exactly one component may eventually consume the event and move
/// payloads such as `Vec<T>` out of it.
pub enum EventOutcome {
    /// The component did not consume the event.
    ///
    /// Ownership is returned so that the event can be offered to another
    /// component.
    Ignored(Box<dyn Event>),

    /// The component consumed the event and may have emitted reactions.
    Consumed(Vec<Box<dyn Event>>),
}

impl EventOutcome {
    /// Creates an ignored outcome.
    pub fn ignored(event: Box<dyn Event>) -> Self {
        Self::Ignored(event)
    }

    pub fn ignored_boxed(event: Box<dyn Event>) -> Self {
        Self::Ignored(event)
    }

    /// Creates a consumed outcome without reactions.
    pub fn consumed() -> Self {
        Self::Consumed(Vec::new())
    }

    /// Creates a consumed outcome with one reaction.
    pub fn consumed_with<E>(event: E) -> Self
    where
        E: Event + 'static,
    {
        Self::Consumed(vec![event.boxed()])
    }

    /// Creates a consumed outcome with boxed reactions.
    pub fn consumed_with_boxed(reactions: Vec<Box<dyn Event>>) -> Self {
        Self::Consumed(reactions)
    }
}

/// Strongly typed, self-describing UI component.
pub trait Component {
    /// Returns this component's unique instance ID.
    fn id(&self) -> ComponentId;

    /// Returns a static string identifying the component type.
    fn kind() -> ComponentKind
    where
        Self: Sized;

    /// Renders the component into the given area.
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>);

    /// Handles an owned event.
    ///
    /// Components should handle only event types they understand. Unknown
    /// events must be returned through [`EventOutcome::Ignored`].
    ///
    /// A component can take ownership of a concrete event without cloning:
    ///
    /// ```rust,ignore
    /// match event.downcast_to::<ItemsLoaded>() {
    ///     Ok(ItemsLoaded(items)) => {
    ///         self.items = items;
    ///         EventOutcome::consumed()
    ///     }
    ///     Err(event) => EventOutcome::Ignored(event),
    /// }
    /// ```
    fn on(&mut self, event: Box<dyn Event>) -> EventOutcome;
}
