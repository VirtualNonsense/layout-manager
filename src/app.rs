use crate::event::{AppEvent, Event, EventHandler};
use crate::ui::{AppCommand, Ui, UiAction};
use crossterm::event::{Event as CrosstermEvent, KeyEvent, KeyEventKind};
use ratatui::{DefaultTerminal, Frame};

pub struct App {
    pub running: bool,
    pub events: EventHandler,
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
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        while self.running {
            terminal.draw(|frame| self.render(frame))?;

            match self.events.next().await? {
                Event::Tick => self.tick(),
                Event::Crossterm(CrosstermEvent::Key(key_event))
                    if key_event.kind == KeyEventKind::Press =>
                {
                    self.handle_key_event(key_event)?;
                }
                Event::Crossterm(CrosstermEvent::Mouse(mouse_event)) => {
                    self.handle_mouse_event(mouse_event)?;
                }
                Event::Crossterm(_) => {}
                Event::App(AppEvent::Quit) => self.quit(),
            }
        }

        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
        self.ui.render(frame, frame.area());
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> color_eyre::Result<()> {
        let actions = self.ui.handle_key_event(key_event);
        self.apply_actions(actions);
        Ok(())
    }

    pub fn handle_mouse_event(
        &mut self,
        mouse_event: crossterm::event::MouseEvent,
    ) -> color_eyre::Result<()> {
        let actions = self.ui.handle_mouse_event(mouse_event);
        self.apply_actions(actions);
        Ok(())
    }

    fn apply_actions(&mut self, actions: Vec<UiAction>) {
        for action in actions {
            match action {
                UiAction::App(AppCommand::Quit) => self.events.send(AppEvent::Quit),
            }
        }
    }

    pub fn tick(&mut self) {}

    pub fn quit(&mut self) {
        self.running = false;
    }
}
