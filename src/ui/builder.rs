use crate::ui::Ui;
use crate::ui::component::{Component, ComponentRegistry};
use crate::ui::focus::FocusManager;
use crate::ui::input::InputManager;
use crate::ui::layout::{ComponentId, LayoutSpec};
use color_eyre::eyre::{Result, eyre};
use std::collections::HashSet;

#[derive(Default)]
pub struct UiBuilder {
    layout: Option<LayoutSpec>,
    components: ComponentRegistry,
    initial_focus: Option<ComponentId>,
    input: Option<InputManager>,
}

impl UiBuilder {
    /// Register a component. Its keybindings are collected here and applied to the
    /// `InputManager` during `build()`.
    pub fn component<C>(mut self, component: C) -> Self
    where
        C: Component + 'static,
    {
        self.components.insert(component);
        self
    }
    pub fn layout(mut self, layout: LayoutSpec) -> Self {
        self.layout = Some(layout);
        self
    }

    pub fn initial_focus(mut self, focus: ComponentId) -> Self {
        self.initial_focus = Some(focus);
        self
    }

    pub fn input(mut self, input: InputManager) -> Self {
        self.input = Some(input);
        self
    }

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
