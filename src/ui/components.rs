use crate::ui::command::{
    ComponentCommand, ContentCommand, PointerEvent, SidebarCommand, UiAction,
};
use crate::ui::layout::ComponentId;
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, BorderType, List, ListItem, ListState, Paragraph},
};
use std::collections::HashMap;
use uuid::Uuid;

pub struct RenderCtx<'a> {
    pub focused: bool,
    pub focus_id: &'a ComponentId,
}

pub enum EventOutcome {
    Ignored,
    Consumed(Vec<UiAction>),
}

/// Strongly typed component trait.
///
/// Each component defines its own command type. The component implementation never needs to match
/// on app-wide command namespaces and cannot accidentally handle another component's command.
pub trait Component {
    type Command: Copy + 'static;

    fn id(&self) -> ComponentId;
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderCtx<'_>);
    fn handle_command(&mut self, command: Self::Command) -> EventOutcome;
    fn component_kind(&self) -> &str;
}

/// Internal object-safe adapter used by the registry.
///
/// This stays private so app code works with the typed [`Component`] trait.
trait ComponentAdapter {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderCtx<'_>);
    fn handle_component_command(&mut self, command: ComponentCommand) -> EventOutcome;
}

impl<T> ComponentAdapter for T
where
    T: Component + 'static,
    T::Command: TryFrom<ComponentCommand, Error = ()>,
{
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderCtx<'_>) {
        Component::render(self, frame, area, ctx);
    }

    fn handle_component_command(&mut self, command: ComponentCommand) -> EventOutcome {
        let Ok(command) = T::Command::try_from(command) else {
            return EventOutcome::Ignored;
        };
        Component::handle_command(self, command)
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
        C::Command: TryFrom<ComponentCommand, Error = ()>,
    {
        let id = Component::id(&component).to_owned();
        self.components.insert(id, Box::new(component));
    }

    pub fn contains(&self, id: &ComponentId) -> bool {
        self.components.contains_key(id)
    }
    pub fn ids(&self) -> impl Iterator<Item = ComponentId> {
        self.components.keys().cloned()
    }

    pub fn render(&mut self, id: &ComponentId, frame: &mut Frame, area: Rect, ctx: RenderCtx<'_>) {
        if let Some(component) = self.components.get_mut(id) {
            component.render(frame, area, ctx);
        }
    }

    pub fn handle_command(&mut self, id: &ComponentId, command: ComponentCommand) -> EventOutcome {
        self.components
            .get_mut(id)
            .map(|component| component.handle_component_command(command))
            .unwrap_or(EventOutcome::Ignored)
    }
}

pub struct SidebarComponent {
    id: ComponentId,
    items: Vec<String>,
    state: ListState,
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
        // local_y includes the border; item 0 starts roughly at y=1 inside the block.
        if let Some(local_y) = event.local_y {
            let index = local_y.saturating_sub(1) as usize;
            if index < self.items.len() {
                self.state.select(Some(index));
            }
        }
    }
}

impl Component for SidebarComponent {
    type Command = SidebarCommand;

    fn id(&self) -> ComponentId {
        self.id
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderCtx<'_>) {
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

    fn handle_command(&mut self, command: SidebarCommand) -> EventOutcome {
        match command {
            SidebarCommand::SelectionUp => self.select_previous(),
            SidebarCommand::SelectionDown => self.select_next(),
            SidebarCommand::Click(event) => self.click(event),
        }
        EventOutcome::Consumed(vec![])
    }

    fn component_kind(&self) -> &str {
        "SidebarComponent"
    }
}

pub struct ContentComponent {
    id: ComponentId,
    counter: i64,
    last_click: Option<PointerEvent>,
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
    type Command = ContentCommand;

    fn id(&self) -> ComponentId {
        self.id
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderCtx<'_>) {
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

    fn handle_command(&mut self, command: ContentCommand) -> EventOutcome {
        match command {
            ContentCommand::CounterInc => self.counter += 1,
            ContentCommand::CounterDec => self.counter -= 1,
            ContentCommand::Click(event) => {
                match event.gesture {
                    super::command::PointerGesture::ScrollUp => self.counter += 1,
                    super::command::PointerGesture::ScrollDown => self.counter -= 1,
                    _ => {}
                }
                self.last_click = Some(event)
            }
        }
        EventOutcome::Consumed(vec![])
    }

    fn component_kind(&self) -> &str {
        "ContentComponent"
    }
}

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
