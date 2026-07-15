//! Validated builder for [`Ui`].
//!
//! [`UiBuilder`] collects all configuration — layout tree, component
//! instances, initial focus, and input manager — and validates the combination
//! at [`build`](UiBuilder::build) time before constructing a [`Ui`].

use crate::ui::Ui;
use crate::ui::component::{Component, ComponentRegistry};
use crate::ui::focus::FocusManager;
use crate::ui::input::InputManager;
use crate::ui::layout::{ComponentId, LayoutSpec};
use color_eyre::eyre::{Result, eyre};
use std::collections::HashSet;

/// Builder for [`Ui`].
///
/// Enforces the following invariants at [`build`](UiBuilder::build) time:
///
/// - The layout must contain at least one leaf.
/// - Every layout leaf must reference a registered component.
/// - All focus IDs across the layout must be unique.
/// - Every registered component must appear somewhere in the layout.
/// - The initial focus ID must be present in the layout.
#[derive(Default)]
pub struct UiBuilder {
    layout: Option<LayoutSpec>,
    components: ComponentRegistry,
    initial_focus: Option<ComponentId>,
    input: Option<InputManager>,
}

impl UiBuilder {
    /// Register a component with the builder.
    ///
    /// The component will be stored in the [`ComponentRegistry`] and must be
    /// referenced by exactly one leaf in the layout passed to
    /// [`layout`](UiBuilder::layout).
    pub fn component<C>(mut self, component: C) -> Self
    where
        C: Component + 'static,
    {
        self.components.insert(component);
        self
    }

    /// Set the layout tree.
    pub fn layout(mut self, layout: LayoutSpec) -> Self {
        self.layout = Some(layout);
        self
    }

    /// Set the component that receives focus when the UI first renders.
    ///
    /// If not set, focus defaults to the first leaf in a depth-first traversal
    /// of the layout tree.
    pub fn initial_focus(mut self, focus: ComponentId) -> Self {
        self.initial_focus = Some(focus);
        self
    }

    /// Override the default [`InputManager`].
    ///
    /// If not set, [`InputManager::default_keymap`] is used.
    pub fn input(mut self, input: InputManager) -> Self {
        self.input = Some(input);
        self
    }

    /// Validate all configuration and construct a [`Ui`].
    ///
    /// Returns an error if any invariant listed on [`UiBuilder`] is violated.
    pub fn build(self) -> Result<Ui> {
        let layout = self.layout.ok_or_else(|| eyre!("UI layout is missing"))?;
        let input = self.input.unwrap_or_else(InputManager::default_keymap);

        let mut leaves = Vec::new();
        layout.collect_leaves(&mut leaves);
        if leaves.is_empty() {
            return Err(eyre!("UI layout must contain at least one leaf"));
        }

        let mut focus_ids = HashSet::new();
        for (component, focus) in &leaves {
            if !self.components.contains(component) {
                return Err(eyre!("layout references missing component '{component}'"));
            }
            if !focus_ids.insert(*(*focus)) {
                return Err(eyre!("duplicate focus id '{focus}'"));
            }
        }

        let initial_focus = self
            .initial_focus
            .or_else(|| leaves.first().map(|(_, focus)| *(*focus)))
            .ok_or_else(|| eyre!("initial focus could not be derived"))?;
        if !focus_ids.contains(&initial_focus) {
            return Err(eyre!(
                "initial focus '{initial_focus}' is not present in the layout"
            ));
        }

        let unused_components: Vec<_> = self
            .components
            .ids()
            .filter(|id| !leaves.iter().any(|(component, _)| *component == id))
            .collect();
        if !unused_components.is_empty() {
            return Err(eyre!(
                "registered components are not used in layout: {:?}",
                unused_components
            ));
        }

        Ok(Ui::from_parts(
            layout,
            self.components,
            FocusManager::new(initial_focus),
            input,
        ))
    }
}
