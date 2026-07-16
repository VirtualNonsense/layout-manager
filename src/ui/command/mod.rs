//! Command types produced by the input layer and consumed by [`Ui`](crate::ui::Ui).
//!
//! The resolution pipeline is:
//! `raw input → InputManager::resolve_* → Command → Ui::dispatch → UiAction`
//!
//! [`Command`] is internal to `Ui`; only [`UiAction`] crosses the boundary
//! into [`App`](crate::app::App).

pub mod app;
pub mod focus;
pub mod pointer;

pub use app::{AppCommand, Direction2D};
pub use focus::FocusCommand;
pub use pointer::{PointerBinding, PointerButton, PointerEvent, PointerGesture};

use crate::event::component::Event;

/// Top-level command produced by the input layer and dispatched by `Ui`.
#[derive(Debug, Clone)]
pub enum Command {
    App(AppCommand),
    Focus(FocusCommand),
    /// Route an abstract event to the currently focused (or pointer-targeted) component.
    Component(Box<dyn Event>),
}

/// An action produced by the UI layer that `App` must act on.
#[derive(Clone, Debug)]
pub enum UiAction {
    App(AppCommand),
    Response(Box<dyn Event>),
}
