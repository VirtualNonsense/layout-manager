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
    key_global: HashMap<KeyStroke, Command>,
    key_component: HashMap<ComponentKind, HashMap<KeyStroke, Command>>,
    pointer_global: HashMap<PointerGesture, PointerBinding>,
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
    pub fn default_keymap() -> Self {
        let mut input = Self::default();

        input.bind_key_component(
            SidebarComponent::kind(),
            KeyCode::Enter,
            KeyModifiers::NONE,
            Command::Component(Box::new(Submit)),
        );

        input.bind_key_global(
            KeyCode::Esc,
            KeyModifiers::NONE,
            Command::App(AppCommand::Quit),
        );
        input.bind_key_global(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            Command::App(AppCommand::Quit),
        );
        input.bind_key_global(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            Command::App(AppCommand::Quit),
        );

        input.bind_key_global(
            KeyCode::Tab,
            KeyModifiers::NONE,
            Command::Focus(FocusCommand::Next),
        );
        input.bind_key_global(
            KeyCode::BackTab,
            KeyModifiers::SHIFT,
            Command::Focus(FocusCommand::Previous),
        );

        input.bind_key_global(
            KeyCode::Up,
            KeyModifiers::ALT,
            Command::Focus(FocusCommand::Move(Direction2D::Up)),
        );
        input.bind_key_global(
            KeyCode::Down,
            KeyModifiers::ALT,
            Command::Focus(FocusCommand::Move(Direction2D::Down)),
        );
        input.bind_key_global(
            KeyCode::Left,
            KeyModifiers::ALT,
            Command::Focus(FocusCommand::Move(Direction2D::Left)),
        );
        input.bind_key_global(
            KeyCode::Right,
            KeyModifiers::ALT,
            Command::Focus(FocusCommand::Move(Direction2D::Right)),
        );
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
        input.bind_key_component(
            ContentComponent::kind(),
            KeyCode::Up,
            KeyModifiers::empty(),
            Command::Component(Box::new(MoveEvent(Direction2D::Up))),
        );
        input.bind_key_component(
            ContentComponent::kind(),
            KeyCode::Down,
            KeyModifiers::empty(),
            Command::Component(Box::new(MoveEvent(Direction2D::Down))),
        );
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
        input.bind_key_component(
            SidebarComponent::kind(),
            KeyCode::Up,
            KeyModifiers::empty(),
            Command::Component(Box::new(MoveEvent(Direction2D::Up))),
        );
        input.bind_key_component(
            SidebarComponent::kind(),
            KeyCode::Down,
            KeyModifiers::empty(),
            Command::Component(Box::new(MoveEvent(Direction2D::Down))),
        );
        input
    }

    /// Register a batch of key and pointer bindings for a given component kind.
    ///
    /// This is the intended API for components to declare their own bindings.
    /// It is not yet called automatically by [`UiBuilder`](crate::ui::builder::UiBuilder);
    /// bindings must currently be added manually in [`default_keymap`](Self::default_keymap).
    pub fn register_component_bindings(
        &mut self,
        kind: ComponentKind,
        key_bindings: &[(KeyCode, KeyModifiers, Command)],
        pointer_bindings: &[(PointerGesture, PointerBinding)],
    ) {
        for (code, modifiers, cmd) in key_bindings {
            let cmd = cmd.clone();
            self.bind_key_component(kind, *code, *modifiers, cmd);
        }

        for (gesture, binding) in pointer_bindings {
            self.bind_pointer_component(kind, *gesture, binding.clone());
        }
    }

    /// Resolve a key event to a [`Command`].
    ///
    /// Checks the component-specific table for `focused` first; falls back to
    /// the global table.  Returns `None` if the key is unbound.
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

        self.key_global.get(&key).cloned()
    }

    /// Resolve a pointer event to a [`Command`].
    ///
    /// Checks the component-specific table for `hovered` first (including a
    /// fallback to the global table); returns `None` if the gesture is unbound.
    ///
    /// When a binding is [`PointerBinding::WithEvent`], the full
    /// [`PointerEvent`] (including component-local coordinates) is wrapped in a
    /// [`MouseEvent`] and forwarded to the component.
    pub fn resolve_pointer(
        &self,
        pointer: PointerEvent,
        hovered: Option<ComponentKind>,
    ) -> Option<Command> {
        let binding = if let Some(kind) = hovered {
            self.pointer_component
                .get(kind)
                .and_then(|bindings| bindings.get(&pointer.gesture))
                .or_else(|| self.pointer_global.get(&pointer.gesture))
        } else {
            self.pointer_global.get(&pointer.gesture)
        }?;

        let cmd = match binding {
            PointerBinding::Fixed(cmd) => Command::Component(cmd.clone()),
            PointerBinding::WithEvent => Command::Component(Box::new(MouseEvent(pointer))),
        };
        Some(cmd)
    }

    /// Add a global key binding.
    pub fn bind_key_global(&mut self, code: KeyCode, modifiers: KeyModifiers, command: Command) {
        self.key_global
            .insert(KeyStroke { code, modifiers }, command);
    }

    /// Add a key binding for a specific component kind.
    pub fn bind_key_component(
        &mut self,
        component: ComponentKind,
        code: KeyCode,
        modifiers: KeyModifiers,
        command: Command,
    ) {
        self.key_component
            .entry(component)
            .or_default()
            .insert(KeyStroke { code, modifiers }, command);
    }

    /// Add a global pointer gesture binding.
    pub fn bind_pointer_global(&mut self, gesture: PointerGesture, binding: PointerBinding) {
        self.pointer_global.insert(gesture, binding);
    }

    /// Add a pointer gesture binding for a specific component kind.
    pub fn bind_pointer_component(
        &mut self,
        component: ComponentKind,
        gesture: PointerGesture,
        binding: PointerBinding,
    ) {
        self.pointer_component
            .entry(component)
            .or_default()
            .insert(gesture, binding);
    }
}
