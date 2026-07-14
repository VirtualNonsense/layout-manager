use ratatui::layout::{Constraint, Direction, Layout, Rect};
use uuid::Uuid;

pub type ComponentId = Uuid;

#[derive(Clone, Debug)]
pub enum LayoutSpec {
    Leaf {
        component: ComponentId,
        focus: ComponentId,
    },
    Split {
        direction: Direction,
        children: Vec<(Constraint, LayoutSpec)>,
    },
}

#[derive(Clone, Debug)]
pub struct LaidOutRegion {
    pub component: ComponentId,
    pub focus: ComponentId,
    pub rect: Rect,
}

impl LayoutSpec {
    pub fn leaf(id: ComponentId) -> Self {
        Self::Leaf {
            component: id,
            focus: id,
        }
    }

    pub fn focused_leaf(component: ComponentId, focus: ComponentId) -> Self {
        Self::Leaf { component, focus }
    }

    pub fn split(direction: Direction, children: Vec<(Constraint, LayoutSpec)>) -> Self {
        Self::Split {
            direction,
            children,
        }
    }

    pub fn compute(&self, area: Rect) -> Vec<LaidOutRegion> {
        let mut out = Vec::new();
        self.compute_into(area, &mut out);
        out
    }

    pub fn collect_leaves<'a>(&'a self, out: &mut Vec<(&'a ComponentId, &'a ComponentId)>) {
        match self {
            LayoutSpec::Leaf { component, focus } => out.push((component, focus)),
            LayoutSpec::Split { children, .. } => {
                for (_, child) in children {
                    child.collect_leaves(out);
                }
            }
        }
    }

    fn compute_into(&self, area: Rect, out: &mut Vec<LaidOutRegion>) {
        match self {
            LayoutSpec::Leaf { component, focus } => out.push(LaidOutRegion {
                component: *component,
                focus: *focus,
                rect: area,
            }),
            LayoutSpec::Split {
                direction,
                children,
            } => {
                let constraints: Vec<Constraint> =
                    children.iter().map(|(constraint, _)| *constraint).collect();
                let chunks = Layout::default()
                    .direction(*direction)
                    .constraints(constraints)
                    .split(area);
                for ((_, child), chunk) in children.iter().zip(chunks.iter()) {
                    child.compute_into(*chunk, out);
                }
            }
        }
    }
}
