//! Focus management.
//!
//! [`FocusManager`] tracks which focus slot currently holds keyboard focus and
//! provides three navigation strategies: direct assignment, cyclic
//! (Tab / Shift-Tab), and geometric (Alt+Arrow).
//!
//! Focus regions are updated on every frame from the output of
//! [`LayoutSpec::compute`](crate::ui::layout::LayoutSpec::compute).

use crate::ui::command::Direction2D;
use crate::ui::layout::ComponentId;
use ratatui::layout::Rect;

/// A focus slot as it exists in screen space.
///
/// Produced by [`Ui`](crate::ui::Ui) each frame from a [`LaidOutRegion`](crate::ui::layout::LaidOutRegion).
#[derive(Clone, Debug)]
pub struct FocusRegion {
    /// The focus identity for this slot (used for focus tracking).
    pub focus: ComponentId,
    /// The component that renders into this slot.
    pub component: ComponentId,
    /// The terminal area occupied by this region.
    pub rect: Rect,
}

/// Tracks which focus slot currently holds keyboard focus.
///
/// Focus regions must be refreshed every frame via [`set_regions`](Self::set_regions)
/// because the terminal may be resized at any time.  If the currently focused
/// ID is no longer present in the new region list, focus automatically resets
/// to the first available region.
#[derive(Default)]
pub struct FocusManager {
    current: Option<ComponentId>,
    regions: Vec<FocusRegion>,
}

impl FocusManager {
    /// Create a new manager with `initial` as the focused ID.
    ///
    /// The ID is accepted even before any regions have been loaded; it will
    /// become active on the first [`set_regions`](Self::set_regions) call that
    /// includes it.
    pub fn new(initial: ComponentId) -> Self {
        Self {
            current: Some(initial),
            regions: Vec::new(),
        }
    }

    /// Return the currently focused focus ID, if any.
    pub fn current(&self) -> Option<ComponentId> {
        self.current
    }

    /// Move focus to `focus` directly.
    ///
    /// The request is silently ignored if `focus` is not in the current region
    /// list.
    pub fn set_current(&mut self, focus: ComponentId) {
        if self.regions.iter().any(|region| region.focus == focus) {
            self.current = Some(focus);
        }
    }

    /// Return the component ID for the currently focused region.
    ///
    /// This differs from [`current`](Self::current) when the component and
    /// focus IDs on a leaf are different (see
    /// [`LayoutSpec::focused_leaf`](crate::ui::layout::LayoutSpec::focused_leaf)).
    pub fn focused_component(&self) -> Option<ComponentId> {
        let current = self.current()?;
        self.regions
            .iter()
            .find(|region| region.focus == current)
            .map(|region| region.component)
    }

    /// Return the region under the given terminal coordinates, if any.
    ///
    /// Used for mouse hit-testing.
    pub fn region_at(&self, x: u16, y: u16) -> Option<&FocusRegion> {
        self.regions
            .iter()
            .find(|region| contains(region.rect, x, y))
    }

    /// Replace the region list with a new snapshot from the current frame.
    ///
    /// If the currently focused ID is absent from the new list, focus resets
    /// to the first region in the new list.
    pub fn set_regions(&mut self, regions: impl IntoIterator<Item = FocusRegion>) {
        self.regions = regions.into_iter().collect();
        if self.current.is_none()
            || self
                .current
                .as_ref()
                .is_some_and(|current| !self.regions.iter().any(|region| &region.focus == current))
        {
            self.current = self.regions.first().map(|region| region.focus);
        }
    }

    /// Move focus to the next region in insertion order (wraps around).
    pub fn next(&mut self) {
        self.offset(1);
    }

    /// Move focus to the previous region in insertion order (wraps around).
    pub fn previous(&mut self) {
        self.offset(-1);
    }

    fn offset(&mut self, delta: isize) {
        if self.regions.is_empty() {
            self.current = None;
            return;
        }
        let index = self
            .current
            .as_ref()
            .and_then(|current| self.regions.iter().position(|r| &r.focus == current))
            .unwrap_or(0);
        let len = self.regions.len() as isize;
        let next = (index as isize + delta).rem_euclid(len) as usize;
        self.current = Some(self.regions[next].focus);
    }

    /// Move focus to the nearest region in the given screen direction.
    ///
    /// A candidate region is considered "in direction" only if its near edge
    /// is at or past the far edge of the current region (no overlap in the
    /// primary axis).  Among valid candidates the one with the lowest
    /// [`navigation_score`] is chosen.
    pub fn move_geometric(&mut self, direction: Direction2D) {
        let Some(current_id) = self.current.as_ref() else {
            return;
        };
        let Some(current) = self
            .regions
            .iter()
            .find(|region| &region.focus == current_id)
        else {
            return;
        };
        let Some(next) = self
            .regions
            .iter()
            .filter(|candidate| candidate.focus != current.focus)
            .filter(|candidate| is_in_direction(current.rect, candidate.rect, direction))
            .min_by_key(|candidate| navigation_score(current.rect, candidate.rect, direction))
        else {
            return;
        };
        self.current = Some(next.focus);
    }
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && y >= rect.y && x < right(rect) && y < bottom(rect)
}

/// Return true if `to` lies entirely in `direction` relative to `from`.
fn is_in_direction(from: Rect, to: Rect, direction: Direction2D) -> bool {
    match direction {
        Direction2D::Up => bottom(to) <= from.y,
        Direction2D::Down => to.y >= bottom(from),
        Direction2D::Left => right(to) <= from.x,
        Direction2D::Right => to.x >= right(from),
    }
}

/// Score a candidate region for geometric navigation.
///
/// Lower is better.  The score combines:
/// - **Primary gap** (distance in the movement direction) × 100 — strongly
///   prefers the closest region along the axis of movement.
/// - **Secondary offset** (center-to-center distance on the perpendicular axis)
///   — breaks ties by preferring regions that are more "aligned".
/// - **+10 000 penalty** when there is no overlap on the perpendicular axis —
///   ensures axially-aligned regions are always preferred over diagonal ones.
fn navigation_score(from: Rect, to: Rect, direction: Direction2D) -> u32 {
    let primary = match direction {
        Direction2D::Up => from.y.saturating_sub(bottom(to)),
        Direction2D::Down => to.y.saturating_sub(bottom(from)),
        Direction2D::Left => from.x.saturating_sub(right(to)),
        Direction2D::Right => to.x.saturating_sub(right(from)),
    } as u32;
    let secondary = match direction {
        Direction2D::Up | Direction2D::Down => center_x(from).abs_diff(center_x(to)) as u32,
        Direction2D::Left | Direction2D::Right => center_y(from).abs_diff(center_y(to)) as u32,
    };
    let overlap_penalty = if axis_overlaps(from, to, direction) {
        0
    } else {
        10_000
    };
    primary * 100 + secondary + overlap_penalty
}

/// Return true if `a` and `b` overlap on the axis perpendicular to `direction`.
fn axis_overlaps(a: Rect, b: Rect, direction: Direction2D) -> bool {
    match direction {
        Direction2D::Up | Direction2D::Down => a.x < right(b) && b.x < right(a),
        Direction2D::Left | Direction2D::Right => a.y < bottom(b) && b.y < bottom(a),
    }
}

fn right(rect: Rect) -> u16 {
    rect.x.saturating_add(rect.width)
}
fn bottom(rect: Rect) -> u16 {
    rect.y.saturating_add(rect.height)
}
fn center_x(rect: Rect) -> u16 {
    rect.x.saturating_add(rect.width / 2)
}
fn center_y(rect: Rect) -> u16 {
    rect.y.saturating_add(rect.height / 2)
}
