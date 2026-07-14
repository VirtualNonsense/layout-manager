use crate::ui::command::{
    ComponentCommand, Direction2D, PointerButton, PointerEvent, PointerGesture, UiAction,
};
use crate::ui::input::PointerBinding;
use crate::ui::layout::ComponentId;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, BorderType, List, ListItem, ListState, Paragraph},
};
use std::collections::HashMap;
use uuid::Uuid;

pub struct RenderContext<'a> {
    pub focused: bool,
    pub focus_id: &'a ComponentId,
}

pub enum EventOutcome {
    Ignored,
    Consumed(Vec<UiAction>),
}

pub type ComponentKind = &'static str;

/// Strongly typed, self-describing component trait.
///
/// Each component defines its own `ComponentCommand`-to-action mapping in `on()` and declares its own
/// keybindings via `key_bindings()` / `pointer_bindings()`. No central command enum exists —
/// adding a new component requires only this file; nothing else needs to be touched.
pub trait Component {
    fn id(&self) -> ComponentId;
    fn kind() -> ComponentKind
    where
        Self: Sized;
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>);

    /// Handle an abstract `ComponentCommand`. Components interpret only the `ComponentCommand` variants they understand
    /// and return `EventOutcome::Ignored` for everything else.
    fn on(&mut self, cmd: ComponentCommand) -> EventOutcome;

    /// Key bindings declared by this component type.
    /// These are registered into the `InputManager` when the component is mounted.
    fn key_bindings() -> &'static [(KeyCode, KeyModifiers, ComponentCommand)]
    where
        Self: Sized;

    /// Pointer bindings declared by this component type.
    fn pointer_bindings() -> &'static [(PointerGesture, PointerBinding)]
    where
        Self: Sized;
}

/// Internal object-safe adapter used by the registry.
///
/// This stays private so app code works with the typed [`Component`] trait.
/// The adapter receives the same abstract `ComponentCommand` that was produced by the input layer —
/// no downcasting or type-erased Any needed: `ComponentCommand` itself is the lingua franca.
trait ComponentAdapter {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>);
    fn on(&mut self, cmd: ComponentCommand) -> EventOutcome;
    fn get_kind(&self) -> ComponentKind;
}

impl<T: Component + 'static> ComponentAdapter for T {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>) {
        Component::render(self, frame, area, ctx);
    }

    fn on(&mut self, cmd: ComponentCommand) -> EventOutcome {
        Component::on(self, cmd)
    }

    fn get_kind(&self) -> ComponentKind {
        T::kind()
    }
}

#[derive(Default)]
pub struct ComponentRegistry {
    components: HashMap<ComponentId, Box<dyn ComponentAdapter>>,
}

impl ComponentRegistry {
    pub fn insert<C>(&mut self, component: C)
    where
        C: Component + 'static,
    {
        let id = component.id();
        self.components.insert(id, Box::new(component));
    }

    pub fn contains(&self, id: &ComponentId) -> bool {
        self.components.contains_key(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.components.keys().copied()
    }

    pub fn render(
        &mut self,
        id: &ComponentId,
        frame: &mut Frame,
        area: Rect,
        ctx: RenderContext<'_>,
    ) {
        if let Some(component) = self.components.get_mut(id) {
            component.render(frame, area, ctx);
        }
    }

    pub fn on(&mut self, id: &ComponentId, cmd: ComponentCommand) -> EventOutcome {
        self.components
            .get_mut(id)
            .map(|component| component.on(cmd))
            .unwrap_or(EventOutcome::Ignored)
    }

    pub fn get_kind(&self, id: &ComponentId) -> Option<ComponentKind> {
        self.components.get(id).map(|c| c.get_kind())
    }
}

// ─── SidebarComponent ────────────────────────────────────────────────────────

pub struct SidebarComponent {
    id: ComponentId,
    items: Vec<String>,
    state: ListState,
}

impl Default for SidebarComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl SidebarComponent {
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            id: Uuid::new_v4(),
            items: vec!["Overview".into(), "Settings".into(), "Logs".into()],
            state,
        }
    }

    fn select_next(&mut self) {
        let selected = self.state.selected().unwrap_or(0);
        let next = (selected + 1).min(self.items.len().saturating_sub(1));
        self.state.select(Some(next));
    }

    fn select_previous(&mut self) {
        let selected = self.state.selected().unwrap_or(0);
        self.state.select(Some(selected.saturating_sub(1)));
    }

    fn click(&mut self, event: PointerEvent) {
        if let Some(local_y) = event.local_y {
            let index = local_y.saturating_sub(1) as usize;
            if index < self.items.len() {
                self.state.select(Some(index));
            }
        }
    }
}

impl Component for SidebarComponent {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn kind() -> ComponentKind {
        "sidebar"
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>) {
        let block = focused_block("Menu", ctx.focused);
        let items = self.items.iter().map(|item| ListItem::new(item.as_str()));
        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("› ");
        frame.render_stateful_widget(list, area, &mut self.state);
    }

    fn on(&mut self, cmd: ComponentCommand) -> EventOutcome {
        match cmd {
            ComponentCommand::Move(Direction2D::Up) => self.select_previous(),
            ComponentCommand::Move(Direction2D::Down) => self.select_next(),
            ComponentCommand::Pointer(event) => self.click(event),
            _ => return EventOutcome::Ignored,
        }
        EventOutcome::Consumed(vec![])
    }

    fn key_bindings() -> &'static [(KeyCode, KeyModifiers, ComponentCommand)] {
        &[
            (
                KeyCode::Up,
                KeyModifiers::NONE,
                ComponentCommand::Move(Direction2D::Up),
            ),
            (
                KeyCode::Down,
                KeyModifiers::NONE,
                ComponentCommand::Move(Direction2D::Down),
            ),
        ]
    }

    fn pointer_bindings() -> &'static [(PointerGesture, PointerBinding)] {
        &[
            (
                PointerGesture::Down(PointerButton::Left),
                PointerBinding::WithEvent,
            ),
            (
                PointerGesture::ScrollUp,
                PointerBinding::Fixed(ComponentCommand::Move(Direction2D::Up)),
            ),
            (
                PointerGesture::ScrollDown,
                PointerBinding::Fixed(ComponentCommand::Move(Direction2D::Down)),
            ),
        ]
    }
}

// ─── ContentComponent ────────────────────────────────────────────────────────

pub struct ContentComponent {
    id: ComponentId,
    counter: i64,
    last_click: Option<PointerEvent>,
}

impl Default for ContentComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentComponent {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            counter: 0,
            last_click: None,
        }
    }
}

impl Component for ContentComponent {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn kind() -> ComponentKind {
        "content"
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>) {
        let click_text = self
            .last_click
            .map(|event| {
                format!(
                    "Last click: global=({}, {}), local=({:?}, {:?})",
                    event.x, event.y, event.local_x, event.local_y
                )
            })
            .unwrap_or_else(|| "Last click: none".to_owned());

        let text = format!(
            "Associated command types + key/mouse input:\n\n\
             q / Esc / Ctrl-C   Quit through original AppEvent channel\n\
             Tab / Shift-Tab    Fokus wechseln\n\
             Alt+Pfeile         Geometrische Fokus-Navigation\n\
             Menu: ↑/↓ + click  Stateful ListState\n\
             Content: ←/→       Counter ändern\n\
             Content: click     Store click coordinates\n\n\
             Focus: {}\n\
             Counter: {}\n\
             {}",
            ctx.focus_id, self.counter, click_text,
        );

        let paragraph = Paragraph::new(text)
            .block(focused_block("Content", ctx.focused))
            .alignment(Alignment::Center)
            .fg(Color::Cyan)
            .bg(Color::Black);
        frame.render_widget(paragraph, area);
    }

    fn on(&mut self, cmd: ComponentCommand) -> EventOutcome {
        match cmd {
            ComponentCommand::Increment => self.counter += 1,
            ComponentCommand::Decrement => self.counter -= 1,
            ComponentCommand::Pointer(event) => {
                match event.gesture {
                    PointerGesture::ScrollUp => self.counter += 1,
                    PointerGesture::ScrollDown => self.counter -= 1,
                    _ => {}
                }
                self.last_click = Some(event);
            }
            _ => return EventOutcome::Ignored,
        }
        EventOutcome::Consumed(vec![])
    }

    fn key_bindings() -> &'static [(KeyCode, KeyModifiers, ComponentCommand)] {
        &[
            (
                KeyCode::Left,
                KeyModifiers::NONE,
                ComponentCommand::Decrement,
            ),
            (
                KeyCode::Right,
                KeyModifiers::NONE,
                ComponentCommand::Increment,
            ),
        ]
    }

    fn pointer_bindings() -> &'static [(PointerGesture, PointerBinding)] {
        &[
            (
                PointerGesture::Down(PointerButton::Left),
                PointerBinding::WithEvent,
            ),
            (PointerGesture::ScrollUp, PointerBinding::WithEvent),
            (PointerGesture::ScrollDown, PointerBinding::WithEvent),
        ]
    }
}

// ─── Shared helpers ──────────────────────────────────────────────────────────

fn focused_block(title: &'static str, focused: bool) -> Block<'static> {
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Block::bordered()
        .title(title)
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
}
