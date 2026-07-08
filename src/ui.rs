pub mod builder;
pub mod command;
pub mod components;
pub mod focus;
pub mod input;
pub mod layout;

use crate::ui::builder::UiBuilder;
use crate::ui::command::{Command, ComponentCommand, FocusCommand, PointerEvent};
use crate::ui::components::{
    Component, ComponentRegistry, ContentComponent, EventOutcome, RenderCtx, SidebarComponent,
};
use crate::ui::focus::{FocusManager, FocusRegion};
use crate::ui::input::{InputContext, InputManager};
use crate::ui::layout::{LaidOutRegion, LayoutSpec};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
};

pub use crate::ui::command::{AppCommand, Direction2D, UiAction};
pub use crate::ui::layout::{ComponentId, FocusId};

pub struct Ui {
    layout: LayoutSpec,
    components: ComponentRegistry,
    focus: FocusManager,
    input: InputManager,
}

impl Ui {
    pub fn builder() -> UiBuilder {
        UiBuilder::default()
    }

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

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let regions = self.layout.compute(area);
        self.update_focus_regions(&regions);

        for region in regions {
            let focused = self.focus.current() == Some(region.focus);
            let ctx = RenderCtx {
                focused,
                focus_id: &region.focus,
            };
            self.components
                .render(&region.component, frame, region.rect, ctx);
        }
    }

    pub fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Vec<UiAction> {
        let context = InputContext {
            focused_component: self.focus.focused_component(),
            hovered_component: None,
        };

        let Some(command) = self.input.resolve_key(key, &context) else {
            return vec![];
        };

        self.dispatch(command)
    }

    pub fn handle_mouse_event(&mut self, mouse: crossterm::event::MouseEvent) -> Vec<UiAction> {
        let hit = self.focus.region_at(mouse.column, mouse.row).cloned();

        // Click-to-focus is runtime behavior, not a component binding.
        if let Some(region) = hit.as_ref()
            && PointerEvent::is_focus_event(mouse.kind)
        {
            self.focus.set_current(region.focus.clone());
        }

        let pointer = PointerEvent::from_mouse_event(mouse, hit.as_ref().map(|r| r.rect));
        let context = InputContext {
            focused_component: self.focus.focused_component(),
            hovered_component: hit.as_ref().map(|region| &region.component),
        };

        let Some(command) = self.input.resolve_pointer(pointer, &context) else {
            return vec![];
        };

        self.dispatch(command)
    }

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
            Command::Component(cmd) => self.dispatch_to_focused_or_targeted_component(cmd),
        }
    }

    fn dispatch_to_focused_or_targeted_component(
        &mut self,
        command: ComponentCommand,
    ) -> Vec<UiAction> {
        // Mouse-originated component commands carry their target component. Key-originated commands
        // do not, so they are routed to the currently focused component.
        let component_id = command
            .target_component()
            .map(str::to_owned)
            .or_else(|| self.focus.focused_component().map(str::to_owned));

        let Some(component_id) = component_id else {
            return vec![];
        };

        match self.components.handle_command(&component_id, command) {
            EventOutcome::Ignored => vec![],
            EventOutcome::Consumed(actions) => actions,
        }
    }

    fn update_focus_regions(&mut self, regions: &[LaidOutRegion]) {
        self.focus
            .set_regions(regions.iter().map(|region| FocusRegion {
                focus: region.focus.clone(),
                component: region.component.clone(),
                rect: region.rect,
            }));
    }
}
