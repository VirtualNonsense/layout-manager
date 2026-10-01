//! Commands produced by the input layer and consumed by crate::ui::Ui.
//!
//! The resolution pipeline is:
//!
//! ```text
//! raw input
//!     -> InputManager::resolve_*
//!     -> Command
//!     -> Ui::dispatch_command
//!     -> Vec<Box<dyn Event>>
//!     -> App queues reactions
//! ```
//!
//! [`Command`] is internal to the UI. Only generated events cross the boundary
//! back into crate::app::App.

pub mod app;
pub mod focus;
pub mod pointer;

pub use app::Direction2D;
pub use focus::FocusCommand;
pub use pointer::{PointerBinding, PointerButton, PointerEvent, PointerGesture};

use crate::event::Event;

/// Top-level command produced by the input layer.
#[derive(Debug, Clone)]
pub enum Command {
    /// Emits an event intended for application-level processing.
    App(Box<dyn Event>),

    /// Performs a focus operation directly within the UI.
    Focus(FocusCommand),

    /// Routes an event to the currently focused component.
    FocusedComponent(Box<dyn Event>),

    /// Clones and broadcasts an event to every component.
    ///
    /// Use only for events whose payload is intentionally cloneable.
    BroadCast(Box<dyn Event>),

    /// Offers an owned event to components until one consumes it.
    ///
    /// No event cloning occurs.
    BroadCastTillConsumed(Box<dyn Event>),
}
