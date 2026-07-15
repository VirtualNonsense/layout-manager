//! Layout tree and region computation.
//!
//! [`LayoutSpec`] describes the desired pane structure as a recursive tree of
//! [`Leaf`](LayoutSpec::Leaf) and [`Split`](LayoutSpec::Split) nodes.
//! Calling [`compute`](LayoutSpec::compute) walks the tree against a concrete
//! terminal [`Rect`] and produces a flat [`Vec<LaidOutRegion>`] that the
//! renderer can iterate directly.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use uuid::Uuid;

/// Unique identifier for a component instance or a focus slot.
///
/// A [`Uuid`] is used so IDs can be generated at component construction time
/// without any central registry.
pub type ComponentId = Uuid;

/// A node in the layout tree.
///
/// # Component ID vs focus ID
///
/// Every `Leaf` carries two IDs:
///
/// - **`component`** — identifies the [`Component`](crate::ui::component::Component)
///   instance that renders into this slot.
/// - **`focus`** — identifies the focus slot that keyboard focus tracks.
///
/// In the common case both are the same; use [`LayoutSpec::leaf`] to create
/// such a node.  They may differ when a single component occupies multiple
/// layout slots and each slot needs an independent focus identity — for example
/// a tabbed pane or a split view over the same underlying component.  Use
/// [`LayoutSpec::focused_leaf`] for that case.
#[derive(Clone, Debug)]
pub enum LayoutSpec {
    /// A terminal node occupied by one component.
    Leaf {
        /// The component that renders into this slot.
        component: ComponentId,
        /// The focus identity for this slot.
        focus: ComponentId,
    },
    /// A node that splits its area among child nodes.
    Split {
        /// Horizontal or vertical split direction.
        direction: Direction,
        /// Ordered list of `(constraint, child)` pairs.
        children: Vec<(Constraint, LayoutSpec)>,
    },
}

/// The result of evaluating one [`LayoutSpec::Leaf`] against a concrete area.
#[derive(Clone, Debug)]
pub struct LaidOutRegion {
    /// The component to render into `rect`.
    pub component: ComponentId,
    /// The focus identity for this region.
    pub focus: ComponentId,
    /// The terminal area assigned to this region.
    pub rect: Rect,
}

impl LayoutSpec {
    /// Create a leaf where the component ID and focus ID are the same.
    ///
    /// This is the standard case: one component, one focus slot.
    pub fn leaf(id: ComponentId) -> Self {
        Self::Leaf {
            component: id,
            focus: id,
        }
    }

    /// Create a leaf with distinct component and focus IDs.
    ///
    /// Use this when one component instance must appear in multiple layout
    /// slots, each with its own independently trackable focus identity.
    pub fn focused_leaf(component: ComponentId, focus: ComponentId) -> Self {
        Self::Leaf { component, focus }
    }

    /// Create a split node that divides its area among children.
    pub fn split(direction: Direction, children: Vec<(Constraint, LayoutSpec)>) -> Self {
        Self::Split {
            direction,
            children,
        }
    }

    /// Walk the tree against `area` and return a flat list of laid-out regions.
    pub fn compute(&self, area: Rect) -> Vec<LaidOutRegion> {
        let mut out = Vec::new();
        self.compute_into(area, &mut out);
        out
    }

    /// Collect all `(component_id, focus_id)` pairs from every leaf in
    /// depth-first order.
    ///
    /// Used by [`UiBuilder`](crate::ui::builder::UiBuilder) to validate the
    /// layout before building.
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
