//! Application-level commands and shared directional type.

/// A cardinal direction in 2D space, used for both focus navigation and component events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction2D {
    Up,
    Down,
    Left,
    Right,
}

/// Top-level application commands that the UI layer can bubble up to `App`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppCommand {
    Quit,
}
