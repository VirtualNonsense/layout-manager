//! Pointer (mouse) event types and bindings.

use crate::{event::Event, new_event};
use crossterm::event::{MouseButton as CrosstermMouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

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

new_event!(PointerEvent {
    gesture: PointerGesture,
    x: u16,
    y: u16,
    local_x: Option<u16>,
    local_y: Option<u16>,
});

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

    pub fn local_position(&self) -> Option<Position> {
        if let (Some(local_x), Some(local_y)) = (self.local_x, self.local_y) {
            return Some(Position::new(local_x, local_y));
        }
        None
    }

    pub fn position(&self) -> Position {
        Position::new(self.x, self.y)
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
