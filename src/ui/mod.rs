//! Central UI coordinator.
//!
//! [`Ui`] ties together all UI sub-systems: the layout tree, focus management,
//! input resolution, and the component registry.  On every frame it recomputes
//! the laid-out regions, updates focus, and renders each component.  Incoming
//! key and mouse events are resolved to [`Command`] values and dispatched
//! through [`dispatch`](Ui::dispatch_to_focused_component).

pub mod builder;
pub mod command;
pub mod component;
pub mod focus;
pub mod input;
pub mod layout;

use crate::event::component::Event;
use crate::ui::builder::UiBuilder;
use crate::ui::command::{Command, FocusCommand, PointerEvent};
use crate::ui::component::{
    Component, ComponentKind, ComponentRegistry, ContentComponent, EventOutcome, RenderContext,
    SidebarComponent,
};
use crate::ui::focus::{FocusManager, FocusRegion};
use crate::ui::input::InputManager;
use crate::ui::layout::{LaidOutRegion, LayoutSpec};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
};

pub use crate::ui::command::{AppCommand, Direction2D, UiAction};
pub use crate::ui::layout::ComponentId;

/// The full UI tree: layout, focus, input bindings, and component instances.
///
/// Construct via [`Ui::builder()`] (validated) or [`Ui::default_ui()`] (the
/// built-in two-pane demo).
pub struct Ui {
    layout: LayoutSpec,
    components: ComponentRegistry,
    focus: FocusManager,
    input: InputManager,
}

impl Ui {
    /// Return a fresh [`UiBuilder`].
    pub fn builder() -> UiBuilder {
        UiBuilder::default()
    }

    /// Build the built-in two-pane demo UI (sidebar + content).
    pub fn default_ui() -> color_eyre::Result<Self> {
        let sidebar_component = SidebarComponent::new();
        let content_component = ContentComponent::new();
        Self::builder()
            .initial_focus(sidebar_component.id())
            .layout(LayoutSpec::split(
                Direction::Horizontal,
                vec![
                    (
                        Constraint::Length(28),
                        LayoutSpec::leaf(sidebar_component.id()),
                    ),
                    (
                        Constraint::Min(20),
                        LayoutSpec::leaf(content_component.id()),
                    ),
                ],
            ))
            .component(sidebar_component)
            .component(content_component)
            .build()
    }

    /// Construct `Ui` directly from its constituent parts.
    ///
    /// Intended for use by [`UiBuilder`] only.
    pub(crate) fn from_parts(
        layout: LayoutSpec,
        components: ComponentRegistry,
        focus: FocusManager,
        input: InputManager,
    ) -> Self {
        Self {
            layout,
            components,
            focus,
            input,
        }
    }

    /// Recompute the layout, update focus regions, and render all components.
    ///
    /// Called once per frame by `App`.
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let regions = self.layout.compute(area);
        self.update_focus_regions(&regions);

        for region in regions {
            let focused = self.focus.current() == Some(region.focus);
            let ctx = RenderContext {
                focused,
                focus_id: &region.focus,
            };
            self.components
                .render(&region.component, frame, region.rect, ctx);
        }
    }

    /// Resolve a key event to a [`Command`] and dispatch it.
    ///
    /// Returns any [`UiAction`] values that `App` must act on.
    pub fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Vec<UiAction> {
        let Some(command) = self.input.resolve_key(key, self.get_focused_kind()) else {
            return vec![];
        };

        self.dispatch(command)
    }

    /// Resolve a mouse event to a [`Command`] and dispatch it.
    ///
    /// A `Down` gesture on any component also transfers focus to that component
    /// before the command is dispatched.  The hovered component (determined by
    /// hit-testing) is used for pointer binding resolution, not the keyboard
    /// focus.
    pub fn handle_mouse_event(&mut self, mouse: crossterm::event::MouseEvent) -> Vec<UiAction> {
        let hit = self.focus.region_at(mouse.column, mouse.row).cloned();

        // Click-to-focus is runtime behavior, not a component binding.
        if let Some(region) = hit.as_ref()
            && PointerEvent::is_focus_event(mouse.kind)
        {
            self.focus.set_current(region.focus);
        }

        let pointer = PointerEvent::from_mouse_event(mouse, hit.as_ref().map(|r| r.rect));
        let hovered = self.get_hovered_kind(hit);

        let Some(command) = self.input.resolve_pointer(pointer, hovered) else {
            return vec![];
        };

        self.dispatch(command)
    }

    /// Dispatch a resolved [`Command`] to the appropriate sub-system.
    fn dispatch(&mut self, command: Command) -> Vec<UiAction> {
        match command {
            Command::App(cmd) => vec![UiAction::App(cmd)],
            Command::Focus(FocusCommand::Move(dir)) => {
                self.focus.move_geometric(dir);
                vec![]
            }
            Command::Focus(FocusCommand::Next) => {
                self.focus.next();
                vec![]
            }
            Command::Focus(FocusCommand::Previous) => {
                self.focus.previous();
                vec![]
            }
            Command::Component(cmd) => self.dispatch_to_focused_component(cmd.as_ref()),
        }
    }

    /// Route a `ComponentCommand` to the currently focused component.
    ///
    /// Mouse-originated `ComponentCommand::Pointer` events are routed to the hovered component via
    /// `resolve_pointer` in `handle_mouse_event` — by the time we get here, the focused
    /// component is already correct (click-to-focus happened above).
    fn dispatch_to_focused_component(&mut self, cmd: &dyn Event) -> Vec<UiAction> {
        let Some(id) = self.focus.focused_component() else {
            return vec![];
        };

        match self.components.on(&id, cmd) {
            EventOutcome::Ignored => vec![],
            EventOutcome::Consumed(actions) => actions,
        }
    }

    fn get_focused_kind(&self) -> Option<ComponentKind> {
        self.focus.focused_component().map(|id| {
            self.components
                .get_kind(&id)
                .expect("focused component must be registered")
        })
    }

    fn get_hovered_kind(&self, hit: Option<FocusRegion>) -> Option<ComponentKind> {
        hit.as_ref().map(|region| {
            self.components
                .get_kind(&region.component)
                .expect("hovered component must be registered")
        })
    }

    fn update_focus_regions(&mut self, regions: &[LaidOutRegion]) {
        self.focus
            .set_regions(regions.iter().map(|region| FocusRegion {
                focus: region.focus,
                component: region.component,
                rect: region.rect,
            }));
    }
}
