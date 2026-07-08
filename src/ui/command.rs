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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarCommand {
    SelectionUp,
    SelectionDown,
    Click(PointerEvent),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentCommand {
    CounterInc,
    CounterDec,
    Click(PointerEvent),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentCommand {
    Sidebar(SidebarCommand),
    Content(ContentCommand),
}

impl ComponentCommand {
    pub fn target_component(&self) -> Option<&'static str> {
        match self {
            ComponentCommand::Sidebar(SidebarCommand::Click(_)) => Some("menu"),
            ComponentCommand::Content(ContentCommand::Click(_)) => Some("content"),
            _ => None,
        }
    }
}

impl TryFrom<ComponentCommand> for SidebarCommand {
    type Error = ();

    fn try_from(value: ComponentCommand) -> Result<Self, Self::Error> {
        match value {
            ComponentCommand::Sidebar(command) => Ok(command),
            _ => Err(()),
        }
    }
}

impl TryFrom<ComponentCommand> for ContentCommand {
    type Error = ();

    fn try_from(value: ComponentCommand) -> Result<Self, Self::Error> {
        match value {
            ComponentCommand::Content(command) => Ok(command),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    App(AppCommand),
    Focus(FocusCommand),
    Component(ComponentCommand),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiAction {
    App(AppCommand),
}
