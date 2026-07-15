use color_eyre::eyre::OptionExt;
use crossterm::event::Event as CrosstermEvent;
use futures::{FutureExt, StreamExt};
use std::{fmt::Debug, time::Duration};
use tokio::sync::mpsc;

use crate::ui::component::component_event::Event;

const TICK_FPS: f64 = 30.0;

#[derive(Debug)]
pub enum EventContainer {
    Tick,
    Quit,
    Crossterm(CrosstermEvent),
    ComponentEvent(Box<dyn Event>),
}

#[derive(Debug)]
pub struct EventHandler {
    sender: mpsc::UnboundedSender<EventContainer>,
    receiver: mpsc::UnboundedReceiver<EventContainer>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let actor = EventTask::new(sender.clone());
        tokio::spawn(async { actor.run().await });
        Self { sender, receiver }
    }

    pub async fn next(&mut self) -> color_eyre::Result<EventContainer> {
        self.receiver
            .recv()
            .await
            .ok_or_eyre("Failed to receive event")
    }

    pub fn send(&mut self, app_event: EventContainer) {
        let _ = self.sender.send(app_event);
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

struct EventTask {
    sender: mpsc::UnboundedSender<EventContainer>,
}

impl EventTask {
    fn new(sender: mpsc::UnboundedSender<EventContainer>) -> Self {
        Self { sender }
    }

    async fn run(self) -> color_eyre::Result<()> {
        let tick_rate = Duration::from_secs_f64(1.0 / TICK_FPS);
        let mut reader = crossterm::event::EventStream::new();
        let mut tick = tokio::time::interval(tick_rate);

        loop {
            let tick_delay = tick.tick();
            let crossterm_event = reader.next().fuse();

            tokio::select! {
                _ = self.sender.closed() => break,
                _ = tick_delay => self.send(EventContainer::Tick),
                Some(Ok(evt)) = crossterm_event => self.send(EventContainer::Crossterm(evt)),
            };
        }

        Ok(())
    }

    fn send(&self, event: EventContainer) {
        let _ = self.sender.send(event);
    }
}
