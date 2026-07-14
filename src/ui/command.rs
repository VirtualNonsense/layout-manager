use crossterm::event::{MouseButton as CrosstermMouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction2D {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppCommand {
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusCommand {
    Move(Direction2D),
    Next,
    Previous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerGesture {
    Down(PointerButton),
    Up(PointerButton),
    Drag(PointerButton),
    Moved,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerEvent {
    pub gesture: PointerGesture,
    pub x: u16,
    pub y: u16,
    pub local_x: Option<u16>,
    pub local_y: Option<u16>,
}

impl PointerEvent {
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

/// Abstract, hardware-agnostic command sent to components.
///
/// Components never see keycodes. The `AppComponent` wrapper layer (the `on()` method on the
/// `Component` trait) is responsible for translating raw input events into `ComponentCommand` values.
/// This means two apps can wire the same component to completely different keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentCommand {
    /// Navigate in a direction (list up/down, text cursor left/right, ...).
    Move(Direction2D),
    /// Confirm / submit the current selection.
    Submit,
    /// A pointer interaction (click, scroll, drag).
    Pointer(PointerEvent),
    /// Increment a numeric value.
    Increment,
    /// Decrement a numeric value.
    Decrement,
}

/// Top-level command produced by the input layer and dispatched by `Ui`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    App(AppCommand),
    Focus(FocusCommand),
    /// Route an abstract `ComponentCommand` to the currently focused (or pointer-targeted) component.
    Component(ComponentCommand),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiAction {
    App(AppCommand),
}
