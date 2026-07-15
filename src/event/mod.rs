//! Async event system.
//!
//! [`EventHandler`] owns an unbounded Tokio mpsc channel.  On construction it
//! spawns an [`EventTask`] that pushes crossterm terminal events and periodic
//! tick events onto the channel.  The main loop calls [`EventHandler::next`] to
//! receive them one at a time.
//!
//! Application code can also inject events directly with
//! [`EventHandler::send`] — this is how `App` converts a `UiAction::Quit`
//! back into `EventContainer::Quit`.

pub mod component;

use color_eyre::eyre::OptionExt;
use crossterm::event::Event as CrosstermEvent;
use futures::{FutureExt, StreamExt};
use std::{fmt::Debug, time::Duration};
use tokio::sync::mpsc;

use crate::event::component::Event;

/// Target render / tick rate in frames per second.
const TICK_FPS: f64 = 30.0;

/// All event kinds that flow through the application channel.
#[derive(Debug)]
pub enum EventContainer {
    /// Periodic timer tick at [`TICK_FPS`] Hz.  Used to drive animations or
    /// time-based state updates.
    Tick,
    /// Requests a clean application shutdown.
    Quit,
    /// A raw crossterm terminal event (key press, mouse, resize, …).
    Crossterm(CrosstermEvent),
    /// A component-originated event that needs to reach `App`.
    ///
    /// This variant is the intended path for components to escalate events
    /// beyond the [`UiAction`](crate::ui::UiAction) boundary.
    /// **Not yet handled** — the dispatch arm in `App::run` is `todo!()`.
    ComponentEvent(Box<dyn Event>),
}

/// Receives [`EventContainer`] values from the background [`EventTask`] and
/// allows application code to inject events imperatively.
#[derive(Debug)]
pub struct EventHandler {
    sender: mpsc::UnboundedSender<EventContainer>,
    receiver: mpsc::UnboundedReceiver<EventContainer>,
}

impl EventHandler {
    /// Create a new handler and spawn the background [`EventTask`].
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let actor = EventTask::new(sender.clone());
        tokio::spawn(async { actor.run().await });
        Self { sender, receiver }
    }

    /// Wait for the next event from the channel.
    ///
    /// Returns an error only if the channel is closed (the background task has
    /// exited unexpectedly).
    pub async fn next(&mut self) -> color_eyre::Result<EventContainer> {
        self.receiver
            .recv()
            .await
            .ok_or_eyre("Failed to receive event")
    }

    /// Push an event onto the channel from application code.
    pub fn send(&mut self, app_event: EventContainer) {
        let _ = self.sender.send(app_event);
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Background Tokio task that produces [`EventContainer`] values.
///
/// Selects over a crossterm [`EventStream`](crossterm::event::EventStream) and
/// a periodic tick timer, forwarding events to the shared channel.  Exits
/// cleanly when the receiver side of the channel is dropped.
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
