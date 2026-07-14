use crate::ui::command::{
    AppCommand, Command, ComponentCommand, Direction2D, FocusCommand, PointerEvent, PointerGesture,
};
use crate::ui::components::ComponentKind;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

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

/// Describes how a pointer gesture maps to a `ComponentCommand` for a component.
///
/// - `Fixed(cmd)`: always produces this `ComponentCommand`, regardless of pointer position.
/// - `WithEvent`:  produces `ComponentCommand::Pointer(event)`, passing full position data to the component.
#[derive(Clone, Copy, Debug)]
pub enum PointerBinding {
    Fixed(ComponentCommand),
    WithEvent,
}

#[derive(Default)]
pub struct InputManager {
    key_global: HashMap<KeyStroke, Command>,
    key_component: HashMap<ComponentKind, HashMap<KeyStroke, Command>>,
    pointer_global: HashMap<PointerGesture, PointerBinding>,
    pointer_component: HashMap<ComponentKind, HashMap<PointerGesture, PointerBinding>>,
}

impl InputManager {
    /// Global bindings only — focus, app commands.
    /// Component-specific bindings are registered via `register_component_bindings()`,
    /// called by the builder when a component is mounted.
    pub fn default_keymap() -> Self {
        let mut input = Self::default();

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

        input
    }

    /// Register a component's self-declared key and pointer bindings.
    ///
    /// Called by `UiBuilder::component()` for every mounted component.
    /// This is the only place where component kind strings and binding tables meet —
    /// the component itself provides both.
    pub fn register_component_bindings(
        &mut self,
        kind: ComponentKind,
        key_bindings: &[(KeyCode, KeyModifiers, ComponentCommand)],
        pointer_bindings: &[(PointerGesture, PointerBinding)],
    ) {
        for (code, modifiers, cmd) in key_bindings {
            let cmd = *cmd;
            self.bind_key_component(kind, *code, *modifiers, Command::Component(cmd));
        }

        for (gesture, binding) in pointer_bindings {
            self.bind_pointer_component(kind, *gesture, *binding);
        }
    }

    pub fn resolve_key(&self, key: KeyEvent, focused: Option<ComponentKind>) -> Option<Command> {
        let key = KeyStroke::from(key);

        if let Some(kind) = focused
            && let Some(command) = self
                .key_component
                .get(kind)
                .and_then(|bindings| bindings.get(&key))
        {
            return Some(*command);
        }

        self.key_global.get(&key).copied()
    }

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
            PointerBinding::Fixed(cmd) => Command::Component(*cmd),
            PointerBinding::WithEvent => Command::Component(ComponentCommand::Pointer(pointer)),
        };
        Some(cmd)
    }

    pub fn bind_key_global(&mut self, code: KeyCode, modifiers: KeyModifiers, command: Command) {
        self.key_global
            .insert(KeyStroke { code, modifiers }, command);
    }

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

    pub fn bind_pointer_global(&mut self, gesture: PointerGesture, binding: PointerBinding) {
        self.pointer_global.insert(gesture, binding);
    }

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
