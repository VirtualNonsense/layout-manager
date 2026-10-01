//! Focus-manipulation commands.

use super::app::Direction2D;

/// Commands that control which component has keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusCommand {
    /// Move focus geometrically in the given direction.
    Move(Direction2D),
    /// Cycle focus to the next component in order.
    Next,
    /// Cycle focus to the previous component in order.
    Previous,
}
