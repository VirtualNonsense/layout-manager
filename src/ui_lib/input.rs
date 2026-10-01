//! Input binding and resolution.
//!
//! [`InputManager`] maintains two lookup tables, one for the whole application
//! and one per [`ComponentKind`], for both key and pointer gestures.
//!
//! Resolution always tries the component-specific table first and then falls
//! back to the global table.

use crate::event::{Event, Quit};
use crate::ui::component::content::MainView;
use crate::ui::component::log_view::LogView;
use crate::ui::component::shortcut_view::ShortCutView;
use crate::ui_lib::command::{
    Command, Direction2D, FocusCommand, PointerBinding, PointerButton, PointerEvent, PointerGesture,
};
use crate::ui_lib::component::{Component, ComponentKind};
use crate::ui_lib::events::{MoveEvent, Submit};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::cmp::Ordering;
use std::collections::HashMap;
use tracing::{Level, instrument, trace};

/// A normalized key press consisting of a key code, modifier mask, and event
/// kind.
///
/// Used as the map key in [`InputManager`]'s key-binding tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyStroke {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
    pub kind: KeyEventKind,
}

impl From<KeyEvent> for KeyStroke {
    fn from(value: KeyEvent) -> Self {
        Self {
            code: value.code,
            modifiers: value.modifiers,
            kind: value.kind,
        }
    }
}

/// Indicates where an available key binding originates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingScope {
    /// The binding belongs to the currently focused component.
    Component,

    /// The binding is available throughout the application.
    Application,
}

/// A user-facing description of an available key binding.
///
/// This type deliberately does not expose the associated [`Command`]. It is
/// intended for informational widgets that display the shortcuts currently
/// available to the user.
#[derive(Clone, Copy, Debug)]
pub struct AvailableKeyBinding {
    pub key: KeyStroke,
    pub description: &'static str,
    pub scope: BindingScope,
}

/// Internal representation of a registered key binding.
#[derive(Clone, Debug)]
struct KeyBinding {
    command: Command,
    description: &'static str,
}

/// Manages key and pointer bindings for the application.
///
/// Bindings are split into two scopes:
///
/// - **Global:** Active regardless of which component is focused or hovered.
/// - **Per-component:** Keyed by [`ComponentKind`] and checked before the
///   global table.
///
/// A component binding shadows a global binding when both use the same
/// [`KeyStroke`].
#[derive(Default, Debug)]
pub struct InputManager {
    key_app: HashMap<KeyStroke, KeyBinding>,
    key_component: HashMap<ComponentKind, HashMap<KeyStroke, KeyBinding>>,
    pointer_app: HashMap<PointerGesture, PointerBinding>,
    pointer_component: HashMap<ComponentKind, HashMap<PointerGesture, PointerBinding>>,
}

impl InputManager {
    /// Builds the default input map used by the application.
    #[instrument(level = "trace")]
    pub fn default_keymap() -> Self {
        let mut input = Self::default();

        /*
         * Global application bindings
         */

        input.bind_app_event(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Quit",
            Command::App(Quit.boxed()),
        );

        input.bind_app_event(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Quit",
            Command::App(Quit.boxed()),
        );

        input.bind_app_event(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            KeyEventKind::Press,
            "Quit",
            Command::App(Quit.boxed()),
        );

        input.bind_app_event(
            KeyCode::Tab,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Focus next",
            Command::Focus(FocusCommand::Next),
        );

        input.bind_app_event(
            KeyCode::BackTab,
            KeyModifiers::SHIFT,
            KeyEventKind::Press,
            "Focus previous",
            Command::Focus(FocusCommand::Previous),
        );

        input.bind_app_event(
            KeyCode::Up,
            KeyModifiers::ALT,
            KeyEventKind::Press,
            "Focus above",
            Command::Focus(FocusCommand::Move(Direction2D::Up)),
        );

        input.bind_app_event(
            KeyCode::Down,
            KeyModifiers::ALT,
            KeyEventKind::Press,
            "Focus below",
            Command::Focus(FocusCommand::Move(Direction2D::Down)),
        );

        input.bind_app_event(
            KeyCode::Left,
            KeyModifiers::ALT,
            KeyEventKind::Press,
            "Focus left",
            Command::Focus(FocusCommand::Move(Direction2D::Left)),
        );

        input.bind_app_event(
            KeyCode::Right,
            KeyModifiers::ALT,
            KeyEventKind::Press,
            "Focus right",
            Command::Focus(FocusCommand::Move(Direction2D::Right)),
        );

        /*
         * MainView bindings
         */

        input.bind_pointer_component(
            MainView::kind(),
            PointerGesture::ScrollUp,
            PointerBinding::WithEvent,
        );

        input.bind_pointer_component(
            MainView::kind(),
            PointerGesture::ScrollDown,
            PointerBinding::WithEvent,
        );

        input.bind_pointer_component(
            MainView::kind(),
            PointerGesture::Down(PointerButton::Left),
            PointerBinding::WithEvent,
        );

        input.bind_key_component(
            MainView::kind(),
            KeyCode::Up,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Move up",
            Command::FocusedComponent(MoveEvent(Direction2D::Up).boxed()),
        );

        input.bind_key_component(
            MainView::kind(),
            KeyCode::Down,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Move down",
            Command::FocusedComponent(MoveEvent(Direction2D::Down).boxed()),
        );

        input.bind_key_component(
            MainView::kind(),
            KeyCode::Left,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Move left",
            Command::FocusedComponent(MoveEvent(Direction2D::Left).boxed()),
        );

        input.bind_key_component(
            MainView::kind(),
            KeyCode::Right,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Move right",
            Command::FocusedComponent(MoveEvent(Direction2D::Right).boxed()),
        );

        input.bind_key_component(
            MainView::kind(),
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Select",
            Command::FocusedComponent(Submit.boxed()),
        );

        /*
         * LogView bindings
         */

        input.bind_pointer_component(
            LogView::kind(),
            PointerGesture::ScrollUp,
            PointerBinding::WithEvent,
        );

        input.bind_pointer_component(
            LogView::kind(),
            PointerGesture::ScrollDown,
            PointerBinding::WithEvent,
        );

        input.bind_pointer_component(
            LogView::kind(),
            PointerGesture::Down(PointerButton::Left),
            PointerBinding::WithEvent,
        );

        input.bind_key_component(
            LogView::kind(),
            KeyCode::Up,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Scroll up",
            Command::FocusedComponent(MoveEvent(Direction2D::Up).boxed()),
        );

        input.bind_key_component(
            LogView::kind(),
            KeyCode::Down,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Scroll down",
            Command::FocusedComponent(MoveEvent(Direction2D::Down).boxed()),
        );

        input.bind_key_component(
            LogView::kind(),
            KeyCode::Right,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Expand selected item",
            Command::FocusedComponent(MoveEvent(Direction2D::Right).boxed()),
        );

        input.bind_key_component(
            LogView::kind(),
            KeyCode::Left,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Collapse",
            Command::FocusedComponent(MoveEvent(Direction2D::Left).boxed()),
        );

        input.bind_key_component(
            LogView::kind(),
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Play / Pause",
            Command::FocusedComponent(Submit.boxed()),
        );

        /*
         * short cut view bindings
         */

        input.bind_pointer_component(
            ShortCutView::kind(),
            PointerGesture::ScrollUp,
            PointerBinding::WithEvent,
        );

        input.bind_pointer_component(
            ShortCutView::kind(),
            PointerGesture::ScrollDown,
            PointerBinding::WithEvent,
        );

        input.bind_key_component(
            ShortCutView::kind(),
            KeyCode::Up,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Select cell above",
            Command::FocusedComponent(MoveEvent(Direction2D::Up).boxed()),
        );

        input.bind_key_component(
            ShortCutView::kind(),
            KeyCode::Down,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            "Select cell bellow",
            Command::FocusedComponent(MoveEvent(Direction2D::Down).boxed()),
        );

        input
    }

    /// Returns the key bindings currently available to the user.
    ///
    /// Component-specific bindings are returned before application bindings.
    /// If the focused component defines the same [`KeyStroke`] as a global
    /// binding, only the component-specific binding is returned.
    #[must_use]
    pub fn available_key_bindings(
        &self,
        focused: Option<ComponentKind>,
    ) -> Vec<AvailableKeyBinding> {
        let component_bindings = focused
            .as_ref()
            .and_then(|kind| self.key_component.get(kind));

        let mut available = Vec::new();

        if let Some(component_bindings) = component_bindings {
            available.extend(
                component_bindings
                    .iter()
                    .map(|(key, binding)| AvailableKeyBinding {
                        key: *key,
                        description: binding.description,
                        scope: BindingScope::Component,
                    }),
            );
        }

        available.extend(
            self.key_app
                .iter()
                .filter(|(key, _)| {
                    component_bindings
                        .map(|bindings| !bindings.contains_key(*key))
                        .unwrap_or(true)
                })
                .map(|(key, binding)| AvailableKeyBinding {
                    key: *key,
                    description: binding.description,
                    scope: BindingScope::Application,
                }),
        );

        available.sort_by(compare_available_bindings);
        available
    }

    /// Resolves a key event to a [`Command`].
    ///
    /// Checks the component-specific table for `focused` first and then falls
    /// back to the global table.
    ///
    /// Returns `None` if the key is unbound.
    #[instrument(skip(self), level = "trace")]
    pub fn resolve_key(&self, key: KeyEvent, focused: Option<ComponentKind>) -> Option<Command> {
        let key = KeyStroke::from(key);

        if let Some(kind) = focused.as_ref()
            && let Some(binding) = self
                .key_component
                .get(kind)
                .and_then(|bindings| bindings.get(&key))
        {
            return Some(binding.command.clone());
        }

        self.key_app
            .get(&key)
            .map(|binding| binding.command.clone())
    }

    /// Resolves a pointer event to a [`Command`].
    ///
    /// Checks the component-specific table for `hovered` first and then falls
    /// back to the global table.
    ///
    /// When a binding is [`PointerBinding::WithEvent`], the complete
    /// [`PointerEvent`] is forwarded to the focused component.
    #[instrument(skip(self), level = "trace")]
    pub fn resolve_pointer(
        &self,
        pointer: PointerEvent,
        hovered: Option<ComponentKind>,
    ) -> Option<Command> {
        let binding = {
            let span = tracing::span!(Level::TRACE, "select_pointer_binding");
            let _guard = span.enter();

            if let Some(kind) = hovered.as_ref() {
                trace!(
                    component = %kind,
                    gesture = ?pointer.gesture,
                    "resolving component pointer binding"
                );

                self.pointer_component
                    .get(kind)
                    .and_then(|bindings| bindings.get(&pointer.gesture))
                    .or_else(|| self.pointer_app.get(&pointer.gesture))
            } else {
                trace!(
                    gesture = ?pointer.gesture,
                    "resolving global pointer binding"
                );

                self.pointer_app.get(&pointer.gesture)
            }?
        };

        let command = match binding {
            PointerBinding::Fixed(command) => Command::FocusedComponent(command.clone()),
            PointerBinding::WithEvent => Command::FocusedComponent(pointer.boxed()),
        };

        Some(command)
    }

    /// Adds a global key binding.
    #[instrument(skip(self, command), level = "trace")]
    pub fn bind_app_event(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        kind: KeyEventKind,
        description: &'static str,
        command: Command,
    ) {
        trace!(
            code = ?code,
            modifiers = ?modifiers,
            event_kind = ?kind,
            description,
            "bound application key"
        );

        self.key_app.insert(
            KeyStroke {
                code,
                modifiers,
                kind,
            },
            KeyBinding {
                command,
                description,
            },
        );
    }

    /// Adds a key binding for a specific component kind.
    #[instrument(skip(self, command), level = "trace")]
    pub fn bind_key_component(
        &mut self,
        component: ComponentKind,
        code: KeyCode,
        modifiers: KeyModifiers,
        kind: KeyEventKind,
        description: &'static str,
        command: Command,
    ) {
        trace!(
            component = %component,
            code = ?code,
            modifiers = ?modifiers,
            event_kind = ?kind,
            description,
            "bound component key"
        );

        self.key_component.entry(component).or_default().insert(
            KeyStroke {
                code,
                modifiers,
                kind,
            },
            KeyBinding {
                command,
                description,
            },
        );
    }

    /// Adds a global pointer gesture binding.
    #[instrument(skip(self, binding), level = "trace")]
    pub fn bind_pointer_app_event(&mut self, gesture: PointerGesture, binding: PointerBinding) {
        trace!(
            gesture = ?gesture,
            binding = ?binding,
            "bound application pointer gesture"
        );

        self.pointer_app.insert(gesture, binding);
    }

    /// Adds a pointer gesture binding for a specific component kind.
    #[instrument(skip(self, binding), level = "trace")]
    pub fn bind_pointer_component(
        &mut self,
        component: ComponentKind,
        gesture: PointerGesture,
        binding: PointerBinding,
    ) {
        trace!(
            component = %component,
            gesture = ?gesture,
            binding = ?binding,
            "bound component pointer gesture"
        );

        self.pointer_component
            .entry(component)
            .or_default()
            .insert(gesture, binding);
    }
}

fn compare_available_bindings(left: &AvailableKeyBinding, right: &AvailableKeyBinding) -> Ordering {
    binding_scope_order(left.scope)
        .cmp(&binding_scope_order(right.scope))
        .then_with(|| key_stroke_order(left.key).cmp(&key_stroke_order(right.key)))
        .then_with(|| left.description.cmp(right.description))
}

fn binding_scope_order(scope: BindingScope) -> u8 {
    match scope {
        BindingScope::Component => 0,
        BindingScope::Application => 1,
    }
}

fn key_stroke_order(key: KeyStroke) -> (u8, u8, String) {
    (
        key_event_kind_order(key.kind),
        modifier_order(key.modifiers),
        key_code_order(key.code),
    )
}

fn key_event_kind_order(kind: KeyEventKind) -> u8 {
    match kind {
        KeyEventKind::Press => 0,
        KeyEventKind::Repeat => 1,
        KeyEventKind::Release => 2,
    }
}

fn modifier_order(modifiers: KeyModifiers) -> u8 {
    let mut order = 0;

    if modifiers.contains(KeyModifiers::CONTROL) {
        order |= 1;
    }

    if modifiers.contains(KeyModifiers::ALT) {
        order |= 2;
    }

    if modifiers.contains(KeyModifiers::SHIFT) {
        order |= 4;
    }

    if modifiers.contains(KeyModifiers::SUPER) {
        order |= 8;
    }

    if modifiers.contains(KeyModifiers::HYPER) {
        order |= 16;
    }

    if modifiers.contains(KeyModifiers::META) {
        order |= 32;
    }

    order
}

fn key_code_order(code: KeyCode) -> String {
    match code {
        KeyCode::Backspace => "00-backspace".to_owned(),
        KeyCode::Enter => "01-enter".to_owned(),
        KeyCode::Left => "02-left".to_owned(),
        KeyCode::Right => "03-right".to_owned(),
        KeyCode::Up => "04-up".to_owned(),
        KeyCode::Down => "05-down".to_owned(),
        KeyCode::Home => "06-home".to_owned(),
        KeyCode::End => "07-end".to_owned(),
        KeyCode::PageUp => "08-page-up".to_owned(),
        KeyCode::PageDown => "09-page-down".to_owned(),
        KeyCode::Tab => "10-tab".to_owned(),
        KeyCode::BackTab => "11-back-tab".to_owned(),
        KeyCode::Delete => "12-delete".to_owned(),
        KeyCode::Insert => "13-insert".to_owned(),
        KeyCode::F(number) => format!("14-f-{number:02}"),
        KeyCode::Char(character) => {
            format!("15-char-{}", character.to_ascii_lowercase())
        }
        KeyCode::Null => "16-null".to_owned(),
        KeyCode::Esc => "17-escape".to_owned(),
        KeyCode::CapsLock => "18-caps-lock".to_owned(),
        KeyCode::ScrollLock => "19-scroll-lock".to_owned(),
        KeyCode::NumLock => "20-num-lock".to_owned(),
        KeyCode::PrintScreen => "21-print-screen".to_owned(),
        KeyCode::Pause => "22-pause".to_owned(),
        KeyCode::Menu => "23-menu".to_owned(),
        KeyCode::KeypadBegin => "24-keypad-begin".to_owned(),
        KeyCode::Media(key) => format!("25-media-{key:?}"),
        KeyCode::Modifier(key) => format!("26-modifier-{key:?}"),
    }
}
