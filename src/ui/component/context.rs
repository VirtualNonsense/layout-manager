//! Per-frame render context and event outcome types.

use crate::ui::command::UiAction;
use crate::ui::layout::ComponentId;

/// The type used to identify a component's kind (for input routing).
pub type ComponentKind = &'static str;

/// Data passed to a component's `render` method on every frame.
pub struct RenderContext<'a> {
    /// Whether this component currently holds keyboard focus.
    pub focused: bool,
    /// The focus-region ID associated with this render slot.
    pub focus_id: &'a ComponentId,
}

/// Returned by a component's `on` handler to indicate whether the event was consumed.
pub enum EventOutcome {
    /// The component did not handle this event.
    Ignored,
    /// The component handled the event and may have produced UI-level actions.
    Consumed(Vec<UiAction>),
}
