//! Top-level application state and main event loop.
//!
//! [`App`] owns the async [`EventHandler`] and the [`Ui`], and drives the
//! render / dispatch cycle.  It is the only place that translates [`UiAction`]
//! values back into [`EventContainer`] messages (e.g. converting
//! `UiAction::App(AppCommand::Quit)` into `EventContainer::Quit`).

use crate::event::{EventContainer, EventHandler};
use crate::log::LogEntry;
use crate::ui::command::Command;
use crate::ui::component::content::ContentComponentEvent;
use crate::ui::component::events::Tick;
use crate::ui::{AppCommand, Ui, UiAction};
use crossterm::event::{Event as CrosstermEvent, KeyEvent, KeyEventKind, MouseEvent};
use ratatui::{DefaultTerminal, Frame};
use tracing::error;

/// Top-level application state.
///
/// Owns the event channel ([`EventHandler`]), the UI tree ([`Ui`]), and the
/// `running` flag that drives the main loop.
pub struct App {
    /// Whether the main loop should keep running.
    pub running: bool,
    /// Async event source (crossterm + tick timer).
    pub events: EventHandler,
    /// The full UI tree (layout, focus, input, components).
    pub ui: Ui,
}

impl Default for App {
    fn default() -> Self {
        Self {
            running: true,
            events: EventHandler::new(),
            ui: Ui::default_ui().expect("built-in UI must be valid"),
        }
    }
}

impl App {
    /// Create a new `App` with the default built-in UI.
    pub fn new() -> Self {
        Self::default()
    }

    /// Run the application loop until `running` is set to `false`.
    ///
    /// Each iteration draws one frame, then blocks on the next
    /// [`EventContainer`]:
    ///
    /// - `Tick` — calls [`tick`](Self::tick) (currently a no-op, reserved for
    ///   time-driven updates).
    /// - `Quit` — calls [`quit`](Self::quit).
    /// - `Crossterm(Key)` — forwards `KeyPress` events to [`Ui`].
    /// - `Crossterm(Mouse)` — forwards mouse events to [`Ui`].
    /// - `ComponentEvent` — **not yet implemented** (`todo!()`); reserved for
    ///   components escalating events past the `UiAction` boundary.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        while self.running {
            terminal.draw(|frame| self.render(frame))?;

            match self.events.next().await? {
                EventContainer::Tick => {
                    let result = self.tick();
                    self.apply_actions(result);
                }
                EventContainer::Quit => self.quit(),
                EventContainer::Crossterm(CrosstermEvent::Key(key_event))
                    if key_event.kind == KeyEventKind::Press =>
                {
                    self.handle_key_event(key_event)?;
                }
                EventContainer::Crossterm(CrosstermEvent::Mouse(mouse_event)) => {
                    self.handle_mouse_event(mouse_event)?;
                }
                EventContainer::Crossterm(_) => {}
                EventContainer::ComponentEvent(_component_event) => todo!(),
                EventContainer::FetchLogs { origin, amount } => {
                    let logs = self.fetch_logs(amount).await;
                    let event = Box::new(ContentComponentEvent::NewLogs(logs));
                    self.ui.dispatch_event_for_component(origin, event);
                }
            }
        }

        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
        self.ui.render(frame, frame.area());
    }

    /// Dispatch a key event to the UI and apply any resulting actions.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> color_eyre::Result<()> {
        let actions = self.ui.handle_key_event(key_event);
        self.apply_actions(actions);
        Ok(())
    }

    /// Dispatch a mouse event to the UI and apply any resulting actions.
    pub fn handle_mouse_event(&mut self, mouse_event: MouseEvent) -> color_eyre::Result<()> {
        let actions = self.ui.handle_mouse_event(mouse_event);
        self.apply_actions(actions);
        Ok(())
    }

    /// Translate [`UiAction`] values produced by the UI layer back into
    /// [`EventContainer`] messages on the event channel.
    fn apply_actions(&mut self, actions: Vec<UiAction>) {
        for action in actions {
            match action {
                UiAction::App(AppCommand::Quit) => self.events.send(EventContainer::Quit),
                UiAction::App(AppCommand::FetchLogs { origin, amount }) => self
                    .events
                    .send(EventContainer::FetchLogs { origin, amount }),
                UiAction::Response(event) => {
                    let event_name = event.event_name();
                    let result = self
                        .ui
                        .dispatch(crate::ui::command::Command::BroadCastTillConsumed(event));
                    if !result.is_empty() {
                        tracing::warn!(
                            "{} did return ui actions: {:?}. this is not allowed due to loops",
                            event_name,
                            result
                        )
                    }
                }
            }
        }
    }

    /// Called on every tick event. Reserved for time-driven updates.
    pub fn tick(&mut self) -> Vec<UiAction> {
        self.ui.dispatch(Command::BroadCast(Box::new(Tick)))
    }

    /// Set `running` to `false`, causing the main loop to exit after the
    /// current iteration.
    pub fn quit(&mut self) {
        self.running = false;
    }

    async fn fetch_logs(&mut self, amount: usize) -> Vec<LogEntry> {
        let logs = tokio::spawn(async move {
            let result = crate::log::tail_log_entries(amount);
            match result {
                Ok(iter) => iter.collect::<Vec<LogEntry>>(),
                Err(e) => {
                    error!("Unable to fetch logs due to {e}");
                    vec![]
                }
            }
        })
        .await;

        match logs {
            Ok(values) => values,
            Err(e) => {
                error!("Unable to fetch logs due to {e}");
                vec![]
            }
        }
    }
}
