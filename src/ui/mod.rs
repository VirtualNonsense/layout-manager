//! Central UI coordinator.
//!
//! [`Ui`] ties together layout, focus management, input resolution, and the
//! component registry.
//!
//! There are two dispatch paths:
//!
//! - [`Ui::dispatch_command`] processes internal input-routing commands.
//! - [`Ui::dispatch_event`] processes owned application event envelopes.

pub mod builder;
pub mod component;
pub mod widget;

use crate::event::{Event, EventEnvelope, EventMetadata};
use crate::ui::builder::UiBuilder;
use crate::ui::component::content::MainView;
use crate::ui::component::log_view::LogView;
use crate::ui::component::shortcut_view::ShortCutView;
use crate::ui_lib::command::{Command, FocusCommand, PointerEvent};
use crate::ui_lib::component::{
    Component, ComponentId, ComponentKind, EventOutcome, RenderContext,
};
use crate::ui_lib::component_registry::ComponentRegistry;
use crate::ui_lib::focus::{FocusManager, FocusRegion};
use crate::ui_lib::input::InputManager;
use crate::ui_lib::layout::{LaidOutRegion, LayoutSpec};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Rect},
};
use tracing::{instrument, trace};

/// Envelope-aware result of dispatching an application event into the UI.
pub enum UiEventOutcome {
    /// No component consumed the event.
    ///
    /// The complete envelope is returned so that the caller retains the
    /// original payload and metadata.
    Ignored(EventEnvelope),

    /// A component consumed the event.
    ///
    /// The original metadata remains available for constructing reaction
    /// envelopes.
    Consumed {
        cause: EventMetadata,
        reactions: Vec<Box<dyn Event>>,
    },
}

impl UiEventOutcome {
    pub fn ignored(envelope: EventEnvelope) -> Self {
        Self::Ignored(envelope)
    }

    pub fn consumed(cause: EventMetadata, reactions: Vec<Box<dyn Event>>) -> Self {
        Self::Consumed { cause, reactions }
    }
}

/// Full UI tree containing layout, focus, input bindings, and components.
pub struct Ui {
    layout: LayoutSpec,
    components: ComponentRegistry,
    focus: FocusManager,
    input: InputManager,
}

impl Ui {
    /// Returns a fresh [`UiBuilder`].
    pub fn builder() -> UiBuilder {
        UiBuilder::default()
    }

    /// Builds the built-in two-pane UI.
    pub fn default_ui() -> color_eyre::Result<Self> {
        let log_view = LogView::new();
        let short_cut_view = ShortCutView::default();
        let content_component = MainView::new();

        Self::builder()
            .initial_focus(short_cut_view.id())
            .layout(LayoutSpec::split(
                Direction::Vertical,
                vec![
                    (
                        Constraint::Min(20),
                        // LayoutSpec::leaf(content_component.id()),
                        LayoutSpec::split(
                            Direction::Horizontal,
                            vec![
                                (Constraint::Fill(1), LayoutSpec::leaf(short_cut_view.id())),
                                (
                                    Constraint::Percentage(50),
                                    LayoutSpec::leaf(content_component.id()),
                                ),
                            ],
                        ),
                    ),
                    (Constraint::Max(10), LayoutSpec::leaf(log_view.id())),
                ],
            ))
            .component(content_component)
            .component(short_cut_view)
            .component(log_view)
            .build()
    }

    /// Constructs `Ui` directly from its constituent parts.
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

    /// Recomputes layout, updates focus regions, and renders components.
    #[instrument(skip(self), level = "trace")]
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let regions = self.layout.compute(area);
        self.update_focus_regions(&regions);

        for region in regions {
            let focused = self.focus.current() == Some(region.focus);
            let focused_kind: Option<ComponentKind> = self
                .focus
                .current()
                .and_then(|id| self.components.get_kind(&id));

            let context = RenderContext {
                focused,
                focus_id: &region.focus,
                manager: &self.input,
                focused_kind,
            };

            self.components
                .render(&region.component, frame, region.rect, context)
        }
    }

    /// Resolves a keyboard input event and dispatches the resulting command.
    #[instrument(skip(self), level = "trace")]
    pub fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Vec<Box<dyn Event>> {
        let Some(command) = self.input.resolve_key(key, self.get_focused_kind()) else {
            return Vec::new();
        };

        self.dispatch_command(command)
    }

    /// Resolves a mouse event and dispatches the resulting command.
    #[instrument(skip(self), level = "trace")]
    pub fn handle_mouse_event(
        &mut self,
        mouse: crossterm::event::MouseEvent,
    ) -> Vec<Box<dyn Event>> {
        let hit = self.focus.region_at(mouse.column, mouse.row).cloned();

        if let Some(region) = hit.as_ref()
            && PointerEvent::is_focus_event(mouse.kind)
        {
            self.focus.set_current(Some(region.focus));
        }

        let pointer = PointerEvent::from_mouse_event(mouse, hit.as_ref().map(|region| region.rect));

        let hovered = self.get_hovered_kind(hit);

        let Some(command) = self.input.resolve_pointer(pointer, hovered) else {
            trace!("pointer input did not resolve to a command");
            return Vec::new();
        };

        self.dispatch_command(command)
    }

    /// Dispatches an internal UI command.
    ///
    /// Returned events are reactions to the input event that produced the
    /// command. `App` is responsible for placing them into `EventQueue`.
    #[instrument(skip(self), level = "trace")]
    pub fn dispatch_command(&mut self, command: Command) -> Vec<Box<dyn Event>> {
        match command {
            Command::App(event) => {
                vec![event]
            }

            Command::Focus(FocusCommand::Move(direction)) => {
                self.focus.move_geometric(direction);
                Vec::new()
            }

            Command::Focus(FocusCommand::Next) => {
                self.focus.next();
                Vec::new()
            }

            Command::Focus(FocusCommand::Previous) => {
                self.focus.previous();
                Vec::new()
            }

            Command::FocusedComponent(event) => self.dispatch_to_focused_component(event),

            Command::BroadCast(event) => self.broadcast_event(event),

            Command::BroadCastTillConsumed(event) => self.broadcast_event_till_consumed(event),
        }
    }

    /// Dispatches an event received from the central event queue.
    ///
    /// The metadata remains outside the component layer. Only the
    /// `Box<dyn Event>` payload is offered to components.
    ///
    /// Application events use first-consumer routing by default, allowing a
    /// component to move fields such as `Vec<T>` out without cloning.
    #[instrument(skip(self), level = "trace")]
    pub fn dispatch_event(&mut self, envelope: EventEnvelope) -> UiEventOutcome {
        let EventEnvelope { metadata, event } = envelope;
        let event_name = event.event_name();

        trace!(
            event = event_name,
            event_id = metadata.id().get(),
            root_id = metadata.root_id().get(),
            depth = metadata.depth(),
            "dispatching queued event through UI"
        );

        match self.components.on_broadcast_till_consumed(event) {
            EventOutcome::Ignored(event) => {
                trace!(
                    event = event_name,
                    event_id = metadata.id().get(),
                    root_id = metadata.root_id().get(),
                    "queued event was ignored by UI"
                );

                UiEventOutcome::Ignored(EventEnvelope::from_parts(metadata, event))
            }

            EventOutcome::Consumed(reactions) => {
                trace!(
                    event = event_name,
                    event_id = metadata.id().get(),
                    root_id = metadata.root_id().get(),
                    reaction_count = reactions.len(),
                    "queued event was consumed by UI"
                );

                UiEventOutcome::Consumed {
                    cause: metadata,
                    reactions,
                }
            }
        }
    }

    /// Dispatches an owned event to one component.
    #[instrument(skip(self))]
    pub fn dispatch_event_for_component(
        &mut self,
        id: ComponentId,
        event: Box<dyn Event>,
    ) -> EventOutcome {
        self.components.on(&id, event)
    }

    /// Routes an event to the currently focused component.
    #[instrument(skip(self), level = "trace")]
    fn dispatch_to_focused_component(&mut self, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        let Some(id) = self.focus.focused_component() else {
            trace!(
                event = event.event_name(),
                "cannot dispatch event because no component is focused"
            );

            return Vec::new();
        };

        let event_name = event.event_name();

        trace!(
            component_id = %id,
            event = event_name,
            "dispatching event to focused component"
        );

        match self.dispatch_event_for_component(id, event) {
            EventOutcome::Ignored(_) => {
                trace!(
                    component_id = %id,
                    event = event_name,
                    "focused component ignored event"
                );

                Vec::new()
            }

            EventOutcome::Consumed(reactions) => reactions,
        }
    }

    /// Offers an event to components until one consumes it.
    #[instrument(skip(self), level = "trace")]
    fn broadcast_event_till_consumed(&mut self, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        match self.components.on_broadcast_till_consumed(event) {
            EventOutcome::Ignored(_) => Vec::new(),
            EventOutcome::Consumed(reactions) => reactions,
        }
    }

    /// Clones an event and broadcasts it to every component.
    #[instrument(skip(self), level = "trace")]
    fn broadcast_event(&mut self, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        self.components.on_broadcast_cloned(event)
    }

    #[instrument(skip(self), level = "trace")]
    fn get_focused_kind(&self) -> Option<ComponentKind> {
        self.focus.focused_component().map(|id| {
            self.components
                .get_kind(&id)
                .expect("focused component must be registered")
        })
    }

    #[instrument(skip(self), level = "trace")]
    fn get_hovered_kind(&self, hit: Option<FocusRegion>) -> Option<ComponentKind> {
        hit.as_ref().map(|region| {
            self.components
                .get_kind(&region.component)
                .expect("hovered component must be registered")
        })
    }

    #[instrument(skip(self, regions), level = "trace")]
    fn update_focus_regions(&mut self, regions: &[LaidOutRegion]) {
        self.focus
            .set_regions(regions.iter().map(|region| FocusRegion {
                focus: region.focus,
                component: region.component,
                rect: region.rect,
            }));
    }
}
