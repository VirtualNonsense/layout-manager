//! Input binding and resolution.
//!
//! [`InputManager`] maintains two lookup tables — one for the whole application
//! (global) and one per [`ComponentKind`] — for both key and pointer gestures.
//! Resolution always tries the component-specific table first, then falls back
//! to the global table.

use crate::ui::command::{
    AppCommand, Command, Direction2D, FocusCommand, PointerBinding, PointerEvent, PointerGesture,
};
use crate::ui::component::events::{MouseEvent, MoveEvent, Submit};
use crate::ui::component::{Component, ComponentKind, ContentComponent, SidebarComponent};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use tracing::{Level, instrument, trace};

use super::command::PointerButton;

/// A normalised key press: key code plus modifier mask.
///
/// Used as the map key in [`InputManager`]'s key binding tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyStroke {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl From<KeyEvent> for KeyStroke {
    fn from(value: KeyEvent) -> Self {
        Self {
            code: value.code,
            modifiers: value.modifiers,
        }
    }
}

/// Manages key and pointer bindings for the application.
///
/// Bindings are split into two scopes:
///
/// - **Global** — active regardless of which component is focused / hovered.
/// - **Per-component** — keyed by [`ComponentKind`] and checked before the
///   global table, allowing components to shadow global bindings for their own
///   keys (e.g. `↑`/`↓` for navigation inside a list).
#[derive(Default)]
pub struct InputManager {
    key_app: HashMap<KeyStroke, Command>,
    key_component: HashMap<ComponentKind, HashMap<KeyStroke, Command>>,
    pointer_app: HashMap<PointerGesture, PointerBinding>,
    pointer_component: HashMap<ComponentKind, HashMap<PointerGesture, PointerBinding>>,
}

impl InputManager {
    /// Build the default keymap used by the demo application.
    ///
    /// Registers global bindings for quit (`q`, `Esc`, `Ctrl-C`), cyclic
    /// focus (`Tab` / `Shift-Tab`), and geometric focus (`Alt+Arrow`).
    ///
    /// Also registers component-specific bindings for `SidebarComponent` and
    /// `ContentComponent` directly in this method.
    ///
    /// > **Note:** Component bindings are currently hard-coded here.
    /// > [`register_component_bindings`](Self::register_component_bindings)
    /// > exists as the intended future API for components to declare their own
    /// > bindings, but it is not yet called automatically by the builder.
    #[instrument(level = "trace")]
    pub fn default_keymap() -> Self {
        let mut input = Self::default();

        input.bind_app_event(
            KeyCode::Esc,
            KeyModifiers::NONE,
            Command::App(AppCommand::Quit),
        );
        input.bind_app_event(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            Command::App(AppCommand::Quit),
        );
        input.bind_app_event(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            Command::App(AppCommand::Quit),
        );

        input.bind_app_event(
            KeyCode::Tab,
            KeyModifiers::NONE,
            Command::Focus(FocusCommand::Next),
        );
        input.bind_app_event(
            KeyCode::BackTab,
            KeyModifiers::SHIFT,
            Command::Focus(FocusCommand::Previous),
        );

        input.bind_app_event(
            KeyCode::Up,
            KeyModifiers::ALT,
            Command::Focus(FocusCommand::Move(Direction2D::Up)),
        );
        input.bind_app_event(
            KeyCode::Down,
            KeyModifiers::ALT,
            Command::Focus(FocusCommand::Move(Direction2D::Down)),
        );
        input.bind_app_event(
            KeyCode::Left,
            KeyModifiers::ALT,
            Command::Focus(FocusCommand::Move(Direction2D::Left)),
        );
        input.bind_app_event(
            KeyCode::Right,
            KeyModifiers::ALT,
            Command::Focus(FocusCommand::Move(Direction2D::Right)),
        );

        // ContentComponent
        input.bind_pointer_component(
            ContentComponent::kind(),
            PointerGesture::ScrollUp,
            PointerBinding::WithEvent,
        );
        input.bind_pointer_component(
            ContentComponent::kind(),
            PointerGesture::ScrollDown,
            PointerBinding::WithEvent,
        );
        input.bind_pointer_component(
            ContentComponent::kind(),
            PointerGesture::Down(PointerButton::Left),
            PointerBinding::WithEvent,
        );
        input.bind_key_component(
            ContentComponent::kind(),
            KeyCode::Up,
            KeyModifiers::empty(),
            Command::FocusedComponent(Box::new(MoveEvent(Direction2D::Up))),
        );
        input.bind_key_component(
            ContentComponent::kind(),
            KeyCode::Down,
            KeyModifiers::empty(),
            Command::FocusedComponent(Box::new(MoveEvent(Direction2D::Down))),
        );
        input.bind_pointer_component(
            ContentComponent::kind(),
            PointerGesture::Down(PointerButton::Left),
            PointerBinding::WithEvent,
        );

        // SidebarComponent
        input.bind_pointer_component(
            SidebarComponent::kind(),
            PointerGesture::ScrollUp,
            PointerBinding::WithEvent,
        );
        input.bind_pointer_component(
            SidebarComponent::kind(),
            PointerGesture::ScrollDown,
            PointerBinding::WithEvent,
        );
        input.bind_pointer_component(
            SidebarComponent::kind(),
            PointerGesture::Down(PointerButton::Left),
            PointerBinding::WithEvent,
        );
        input.bind_key_component(
            SidebarComponent::kind(),
            KeyCode::Up,
            KeyModifiers::empty(),
            Command::FocusedComponent(Box::new(MoveEvent(Direction2D::Up))),
        );
        input.bind_key_component(
            SidebarComponent::kind(),
            KeyCode::Down,
            KeyModifiers::empty(),
            Command::FocusedComponent(Box::new(MoveEvent(Direction2D::Down))),
        );
        input.bind_key_component(
            SidebarComponent::kind(),
            KeyCode::Enter,
            KeyModifiers::NONE,
            Command::FocusedComponent(Box::new(Submit)),
        );
        input
    }

    /// Resolve a key event to a [`Command`].
    ///
    /// Checks the component-specific table for `focused` first; falls back to
    /// the global table.  Returns `None` if the key is unbound.
    #[instrument(skip(self), level = "trace")]
    pub fn resolve_key(&self, key: KeyEvent, focused: Option<ComponentKind>) -> Option<Command> {
        let key = KeyStroke::from(key);

        if let Some(kind) = focused
            && let Some(command) = self
                .key_component
                .get(kind)
                .and_then(|bindings| bindings.get(&key))
        {
            return Some(command.clone());
        }

        self.key_app.get(&key).cloned()
    }

    /// Resolve a pointer event to a [`Command`].
    ///
    /// Checks the component-specific table for `hovered` first (including a
    /// fallback to the global table); returns `None` if the gesture is unbound.
    ///
    /// When a binding is [`PointerBinding::WithEvent`], the full
    /// [`PointerEvent`] (including component-local coordinates) is wrapped in a
    /// [`MouseEvent`] and forwarded to the component.
    #[instrument(skip(self), level = "trace")]
    pub fn resolve_pointer(
        &self,
        pointer: PointerEvent,
        hovered: Option<ComponentKind>,
    ) -> Option<Command> {
        let binding = {
            let span = tracing::span!(Level::TRACE, "select_binding");
            let _enter = span.enter();
            if let Some(kind) = hovered {
                trace!("hovered: {kind}");
                self.pointer_component
                    .get(kind)
                    .and_then(|bindings| bindings.get(&pointer.gesture))
                    .or_else(|| self.pointer_app.get(&pointer.gesture))
            } else {
                trace!("resolving pointer binding instead");
                self.pointer_app.get(&pointer.gesture)
            }?
        };

        let cmd = match binding {
            PointerBinding::Fixed(cmd) => Command::FocusedComponent(cmd.clone()),
            PointerBinding::WithEvent => Command::FocusedComponent(Box::new(MouseEvent(pointer))),
        };
        Some(cmd)
    }

    /// Add a global key binding.
    #[instrument(skip(self), level = "trace")]
    pub fn bind_app_event(&mut self, code: KeyCode, modifiers: KeyModifiers, command: Command) {
        trace!("bound {:?} to {} with {} for app", command, code, modifiers);
        self.key_app.insert(KeyStroke { code, modifiers }, command);
    }

    /// Add a key binding for a specific component kind.
    #[instrument(skip(self), level = "trace")]
    pub fn bind_key_component(
        &mut self,
        component: ComponentKind,
        code: KeyCode,
        modifiers: KeyModifiers,
        command: Command,
    ) {
        trace!(
            "bound {:?} to {} with {} for {}",
            command, code, modifiers, component
        );
        self.key_component
            .entry(component)
            .or_default()
            .insert(KeyStroke { code, modifiers }, command);
    }

    /// Add a global pointer gesture binding.
    #[instrument(skip(self), level = "trace")]
    pub fn bind_pointer_app_event(&mut self, gesture: PointerGesture, binding: PointerBinding) {
        trace!("bound {:?} to {:?}  for app", binding, gesture);
        trace!("bound {binding:?} to {gesture:?} for the entire app");
        self.pointer_app.insert(gesture, binding);
    }

    /// Add a pointer gesture binding for a specific component kind.
    #[instrument(skip(self), level = "trace")]
    pub fn bind_pointer_component(
        &mut self,
        component: ComponentKind,
        gesture: PointerGesture,
        binding: PointerBinding,
    ) {
        trace!("bound {:?} to {:?} for {}", binding, gesture, component);
        self.pointer_component
            .entry(component)
            .or_default()
            .insert(gesture, binding);
    }
}
