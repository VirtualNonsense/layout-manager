//! Component trait and supporting types.
//!
//! Every UI pane is a [`Component`].  Implement this trait to create a new
//! component; nothing else in the framework needs to be modified.  The
//! component is registered with [`UiBuilder::component`](crate::ui::builder::UiBuilder::component)
//! and placed in the layout tree via a [`LayoutSpec`](crate::ui::layout::LayoutSpec) leaf.

pub mod context;
pub mod events;
pub mod registry;
pub mod widgets;

pub mod content;
pub mod sidebar;

pub use content::ContentComponent;
pub use context::{ComponentKind, EventOutcome, RenderContext};
pub use registry::ComponentRegistry;
pub use sidebar::SidebarComponent;

use crate::event::component::Event;
use crate::ui::layout::ComponentId;
use ratatui::{Frame, layout::Rect};

/// Strongly typed, self-describing component trait.
///
/// Implement this for every UI component. The component declares its own kind string
/// (`kind()`) which is used as the key for input routing in [`InputManager`](crate::ui::input::InputManager).
///
/// Add a new component by implementing this trait — nothing else in the framework
/// needs to be modified.
pub trait Component {
    /// Returns this component's unique instance ID.
    fn id(&self) -> ComponentId;

    /// Returns a static string identifying the component type (used for input routing).
    fn kind() -> ComponentKind
    where
        Self: Sized;

    /// Render the component into the given area.
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>);

    /// Handle an event routed by the input layer.
    ///
    /// Components should handle only the event types they understand and return
    /// [`EventOutcome::Ignored`] for everything else.
    fn on(&mut self, event: Box<dyn Event>) -> EventOutcome;
}
