use crate::ui::command::Direction2D;
use crate::ui::layout::ComponentId;
use ratatui::layout::Rect;

#[derive(Clone, Debug)]
pub struct FocusRegion {
    pub focus: ComponentId,
    pub component: ComponentId,
    pub rect: Rect,
}

#[derive(Default)]
pub struct FocusManager {
    current: Option<ComponentId>,
    regions: Vec<FocusRegion>,
}

impl FocusManager {
    pub fn new(initial: ComponentId) -> Self {
        Self {
            current: Some(initial.into()),
            regions: Vec::new(),
        }
    }

    pub fn current(&self) -> Option<ComponentId> {
        self.current
    }

    pub fn set_current(&mut self, focus: ComponentId) {
        let focus = focus.into();
        if self.regions.iter().any(|region| region.focus == focus) {
            self.current = Some(focus);
        }
    }

    pub fn focused_component(&self) -> Option<ComponentId> {
        let current = self.current()?;
        self.regions
            .iter()
            .find(|region| region.focus == current)
            .map(|region| region.component)
    }

    pub fn region_at(&self, x: u16, y: u16) -> Option<&FocusRegion> {
        self.regions
            .iter()
            .find(|region| contains(region.rect, x, y))
    }

    pub fn set_regions(&mut self, regions: impl IntoIterator<Item = FocusRegion>) {
        self.regions = regions.into_iter().collect();
        if self.current.is_none()
            || self
                .current
                .as_ref()
                .is_some_and(|current| !self.regions.iter().any(|region| &region.focus == current))
        {
            self.current = self.regions.first().map(|region| region.focus.clone());
        }
    }

    pub fn next(&mut self) {
        self.offset(1);
    }
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
        self.current = Some(self.regions[next].focus.clone());
    }

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
        self.current = Some(next.focus.clone());
    }
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && y >= rect.y && x < right(rect) && y < bottom(rect)
}

fn is_in_direction(from: Rect, to: Rect, direction: Direction2D) -> bool {
    match direction {
        Direction2D::Up => bottom(to) <= from.y,
        Direction2D::Down => to.y >= bottom(from),
        Direction2D::Left => right(to) <= from.x,
        Direction2D::Right => to.x >= right(from),
    }
}

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
