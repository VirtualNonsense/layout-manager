//! Pointer (mouse) event types and bindings.

use crate::event::component::Event;
use crossterm::event::{MouseButton as CrosstermMouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

/// A mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

/// A normalised mouse gesture, used as the key in pointer binding tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerGesture {
    /// Button pressed.
    Down(PointerButton),
    /// Button released.
    Up(PointerButton),
    /// Button held while moving.
    Drag(PointerButton),
    /// Cursor moved (no button held).
    Moved,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
}

/// A pointer event with both global and component-local coordinates.
///
/// `local_x` / `local_y` are relative to the top-left corner of the component
/// rect that the pointer was over.  They are `None` when the pointer was not
/// inside any component rect at the time of the event (e.g. the cursor moved
/// off-screen or into a gap).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerEvent {
    /// The normalised gesture that triggered this event.
    pub gesture: PointerGesture,
    /// Absolute terminal column.
    pub x: u16,
    /// Absolute terminal row.
    pub y: u16,
    /// Column relative to the target component's rect, or `None` if outside.
    pub local_x: Option<u16>,
    /// Row relative to the target component's rect, or `None` if outside.
    pub local_y: Option<u16>,
}

impl PointerEvent {
    /// Convert a crossterm [`MouseEvent`] into a [`PointerEvent`].
    ///
    /// If `target` is provided and the cursor position falls within it,
    /// `local_x` / `local_y` are computed relative to its top-left corner.
    pub fn from_mouse_event(event: MouseEvent, target: Option<Rect>) -> Self {
        let gesture = PointerGesture::from(event.kind);
        let (local_x, local_y) = target
            .filter(|rect| contains(*rect, event.column, event.row))
            .map(|rect| {
                (
                    Some(event.column.saturating_sub(rect.x)),
                    Some(event.row.saturating_sub(rect.y)),
                )
            })
            .unwrap_or((None, None));

        Self {
            gesture,
            x: event.column,
            y: event.row,
            local_x,
            local_y,
        }
    }

    /// Return `true` if `kind` should transfer keyboard focus to the clicked
    /// component.
    ///
    /// Currently only `MouseDown` triggers a focus transfer.
    pub fn is_focus_event(kind: MouseEventKind) -> bool {
        matches!(kind, MouseEventKind::Down(_))
    }
}

impl From<MouseEventKind> for PointerGesture {
    fn from(value: MouseEventKind) -> Self {
        match value {
            MouseEventKind::Down(button) => PointerGesture::Down(button.into()),
            MouseEventKind::Up(button) => PointerGesture::Up(button.into()),
            MouseEventKind::Drag(button) => PointerGesture::Drag(button.into()),
            MouseEventKind::Moved => PointerGesture::Moved,
            MouseEventKind::ScrollUp => PointerGesture::ScrollUp,
            MouseEventKind::ScrollDown => PointerGesture::ScrollDown,
            MouseEventKind::ScrollLeft => PointerGesture::ScrollLeft,
            MouseEventKind::ScrollRight => PointerGesture::ScrollRight,
        }
    }
}

impl From<CrosstermMouseButton> for PointerButton {
    fn from(value: CrosstermMouseButton) -> Self {
        match value {
            CrosstermMouseButton::Left => PointerButton::Left,
            CrosstermMouseButton::Right => PointerButton::Right,
            CrosstermMouseButton::Middle => PointerButton::Middle,
        }
    }
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x
        && y >= rect.y
        && x < rect.x.saturating_add(rect.width)
        && y < rect.y.saturating_add(rect.height)
}

/// Describes how a pointer gesture maps to a component event.
///
/// - `Fixed(event)`: always produces this event, regardless of pointer position.
/// - `WithEvent`: produces a `MouseEvent(pointer)`, passing full position data to the component.
#[derive(Clone, Debug)]
pub enum PointerBinding {
    Fixed(Box<dyn Event>),
    WithEvent,
}
