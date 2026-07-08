use crate::ui::{
    ComponentId,
    command::{
        AppCommand, Command, ComponentCommand, ContentCommand, Direction2D, FocusCommand,
        PointerButton, PointerEvent, PointerGesture, SidebarCommand,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyStroke {
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl From<KeyEvent> for KeyStroke {
    fn from(value: KeyEvent) -> Self {
        Self {
            code: value.code,
            modifiers: value.modifiers,
        }
    }
}

pub struct InputContext {
    pub focused_component: Option<ComponentId>,
    pub hovered_component: Option<ComponentId>,
}

type PointerCommandFactory = fn(PointerEvent) -> Command;

pub struct ComponentPath {}

#[derive(Default)]
pub struct InputManager {
    key_global: HashMap<KeyStroke, Command>,
    key_component: HashMap<ComponentId, HashMap<KeyStroke, Command>>,
    pointer_global: HashMap<PointerGesture, PointerCommandFactory>,
    pointer_component: HashMap<ComponentId, HashMap<PointerGesture, PointerCommandFactory>>,
}

impl InputManager {
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

        input.bind_key_component(
            "menu",
            KeyCode::Up,
            KeyModifiers::NONE,
            Command::Component(ComponentCommand::Sidebar(SidebarCommand::SelectionUp)),
        );
        input.bind_key_component(
            "menu",
            KeyCode::Down,
            KeyModifiers::NONE,
            Command::Component(ComponentCommand::Sidebar(SidebarCommand::SelectionDown)),
        );
        input.bind_key_component(
            "content",
            KeyCode::Left,
            KeyModifiers::NONE,
            Command::Component(ComponentCommand::Content(ContentCommand::CounterDec)),
        );
        input.bind_key_component(
            "content",
            KeyCode::Right,
            KeyModifiers::NONE,
            Command::Component(ComponentCommand::Content(ContentCommand::CounterInc)),
        );

        input.bind_pointer_component("content", PointerGesture::ScrollUp, |event| {
            Command::Component(ComponentCommand::Content(ContentCommand::Click(event)))
        });
        input.bind_pointer_component("content", PointerGesture::ScrollDown, |event| {
            Command::Component(ComponentCommand::Content(ContentCommand::Click(event)))
        });

        input.bind_pointer_component("menu", PointerGesture::Down(PointerButton::Left), |event| {
            Command::Component(ComponentCommand::Sidebar(SidebarCommand::Click(event)))
        });
        input.bind_pointer_component("menu", PointerGesture::ScrollUp, |_| {
            Command::Component(ComponentCommand::Sidebar(SidebarCommand::SelectionUp))
        });
        input.bind_pointer_component("menu", PointerGesture::ScrollDown, |_| {
            Command::Component(ComponentCommand::Sidebar(SidebarCommand::SelectionDown))
        });
        input.bind_pointer_component(
            "content",
            PointerGesture::Down(PointerButton::Left),
            |event| Command::Component(ComponentCommand::Content(ContentCommand::Click(event))),
        );

        input
    }

    pub fn resolve_key(&self, key: KeyEvent, context: &InputContext<'_>) -> Option<Command> {
        let key = KeyStroke::from(key);

        if let Some(component_id) = context.focused_component
            && let Some(command) = self
                .key_component
                .get(component_id)
                .and_then(|bindings| bindings.get(&key))
        {
            return Some(*command);
        }

        self.key_global.get(&key).copied()
    }

    pub fn resolve_pointer(
        &self,
        pointer: PointerEvent,
        context: &InputContext<'_>,
    ) -> Option<Command> {
        if let Some(component_id) = context.hovered_component
            && let Some(factory) = self
                .pointer_component
                .get(component_id)
                .and_then(|bindings| bindings.get(&pointer.gesture))
        {
            return Some(factory(pointer));
        }

        self.pointer_global
            .get(&pointer.gesture)
            .map(|factory| factory(pointer))
    }

    pub fn bind_key_global(&mut self, code: KeyCode, modifiers: KeyModifiers, command: Command) {
        self.key_global
            .insert(KeyStroke { code, modifiers }, command);
    }

    pub fn bind_key_component(
        &mut self,
        component: ComponentId,
        code: KeyCode,
        modifiers: KeyModifiers,
        command: Command,
    ) {
        self.key_component
            .entry(component.into())
            .or_default()
            .insert(KeyStroke { code, modifiers }, command);
    }

    pub fn bind_pointer_global(&mut self, gesture: PointerGesture, factory: PointerCommandFactory) {
        self.pointer_global.insert(gesture, factory);
    }

    pub fn bind_pointer_component(
        &mut self,
        component: ComponentId,
        gesture: PointerGesture,
        factory: PointerCommandFactory,
    ) {
        self.pointer_component
            .entry(component.into())
            .or_default()
            .insert(gesture, factory);
    }
}
