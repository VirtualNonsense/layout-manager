//! Content component that displays the application's recent log entries.

use std::time::{Duration, Instant};

use ratatui::{Frame, layout::Rect};
use tokio::sync::oneshot;
use tracing::{debug, error, instrument};
use uuid::Uuid;

use crate::{
    event::{Event, Tick},
    log::LogEntry,
    ui::{
        component::widgets::focused_block,
        widget::log::{LogViewState, LogViewWidget},
    },
    ui_lib::{
        command::{Direction2D, PointerEvent, PointerGesture},
        component::{Component, ComponentId, ComponentKind, EventOutcome, RenderContext},
        events::{MoveEvent, Submit},
    },
};

/// Maximum number of recent log entries fetched from the log file.
const DEFAULT_LOG_ENTRY_COUNT: usize = 100;

/// Minimum interval between background update requests.
const DEFAULT_UPDATE_INTERVAL: Duration = Duration::from_secs(1);

pub struct LogView {
    id: ComponentId,

    /// Selection, scrolling, pointer geometry, and expansion state.
    state: LogViewState,

    /// Maximum number of recent log entries displayed.
    log_entry_count: usize,

    /// Most recently fetched log entries.
    logs: Vec<LogEntry>,

    /// Receiver for the active asynchronous log-fetch operation.
    log_update: Option<oneshot::Receiver<Vec<LogEntry>>>,

    /// Whether automatic log updates are currently paused.
    log_updates_paused: bool,

    /// Time at which the most recent log update was requested or received.
    last_log_update: Option<Instant>,

    /// Minimum interval between update requests.
    update_interval: Duration,
}

impl Default for LogView {
    fn default() -> Self {
        Self::new()
    }
}

impl LogView {
    /// Creates the log-view component.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            state: LogViewState::new(),
            log_entry_count: DEFAULT_LOG_ENTRY_COUNT,
            logs: Vec::new(),
            log_update: None,
            log_updates_paused: false,
            last_log_update: None,
            update_interval: DEFAULT_UPDATE_INTERVAL,
        }
    }

    /// Handles directional navigation.
    fn handle_log_movement(&mut self, direction: Direction2D) -> EventOutcome {
        let entry_count = self.logs.len();

        match direction {
            Direction2D::Up => {
                self.state.list.select_previous(entry_count);
            }
            Direction2D::Down => {
                self.state.list.select_next(entry_count);
            }
            Direction2D::Right => {
                self.state.expand_selected();
            }
            Direction2D::Left => {
                self.state.collapse_selected();
            }
        }

        EventOutcome::Consumed(Vec::new())
    }

    /// Handles pointer input.
    fn handle_pointer_event(&mut self, pointer: &PointerEvent) -> Option<EventOutcome> {
        match pointer.gesture {
            PointerGesture::ScrollUp => {
                self.state.list.select_previous(self.logs.len());

                Some(EventOutcome::Consumed(Vec::new()))
            }

            PointerGesture::ScrollDown => {
                self.state.list.select_next(self.logs.len());

                Some(EventOutcome::Consumed(Vec::new()))
            }

            PointerGesture::Down(_) => {
                let index = self
                    .state
                    .index_at_pointer(pointer.position(), &self.logs)?;

                if self.state.selected() == Some(index) {
                    self.state.toggle(index);
                } else {
                    self.state.select(index);
                }

                Some(EventOutcome::Consumed(Vec::new()))
            }

            _ => None,
        }
    }

    /// Toggles automatic log updates.
    ///
    /// Resuming automatic updates immediately schedules a new fetch.
    fn toggle_log_updates(&mut self) {
        self.log_updates_paused = !self.log_updates_paused;

        if self.log_updates_paused {
            debug!("automatic log updates paused");
            return;
        }

        debug!("automatic log updates resumed");

        self.last_log_update = None;
        self.request_log_update();
    }

    /// Starts fetching the most recent log entries.
    ///
    /// If updates are paused or a fetch is already active, no new operation is
    /// started.
    fn request_log_update(&mut self) {
        if self.log_updates_paused || self.log_update.is_some() {
            return;
        }

        let amount = self.log_entry_count;
        let (sender, receiver) = oneshot::channel();

        self.log_update = Some(receiver);
        self.last_log_update = Some(Instant::now());

        tokio::task::spawn_blocking(move || {
            let logs = match crate::log::tail_log_entries(amount) {
                Ok(entries) => entries.collect::<Vec<LogEntry>>(),

                Err(error) => {
                    error!(
                        error = ?error,
                        amount,
                        "unable to fetch log entries"
                    );

                    Vec::new()
                }
            };

            if sender.send(logs).is_err() {
                debug!(
                    amount,
                    "discarding fetched log entries because the receiver was dropped"
                );
            }
        });
    }

    /// Polls the active log-fetch operation.
    ///
    /// This continues polling while automatic updates are paused so an
    /// operation that was already running can still complete.
    fn poll_log_update(&mut self) {
        let Some(receiver) = self.log_update.as_mut() else {
            return;
        };

        match receiver.try_recv() {
            Ok(logs) => {
                self.log_update = None;
                self.last_log_update = Some(Instant::now());
                self.logs = logs;

                let entry_count = self.logs.len();

                self.state.normalize(entry_count);

                if self.state.selected().is_none() && entry_count > 0 {
                    self.state.list.select_first(entry_count);
                }
            }

            Err(oneshot::error::TryRecvError::Closed) => {
                self.log_update = None;
                self.last_log_update = Some(Instant::now());

                error!("log-update channel closed before producing a result");
            }

            Err(oneshot::error::TryRecvError::Empty) => {}
        }
    }

    /// Returns whether the configured update interval has elapsed.
    fn update_interval_elapsed(&self, last_update: Option<Instant>) -> bool {
        last_update.is_none_or(|last_update| last_update.elapsed() >= self.update_interval)
    }

    /// Handles a periodic application tick.
    fn handle_tick(&mut self, tick: Tick) -> EventOutcome {
        self.poll_log_update();

        if !self.log_updates_paused && self.update_interval_elapsed(self.last_log_update) {
            self.request_log_update();
        }

        EventOutcome::Ignored(tick.boxed())
    }

    /// Returns the title displayed above the log view.
    fn title(&self) -> &'static str {
        if self.log_updates_paused {
            "Logs [PAUSED]"
        } else {
            "Logs [LIVE]"
        }
    }

    #[instrument(skip(self, event))]
    fn handle_event(&mut self, mut event: Box<dyn Event>) -> EventOutcome {
        event = match event.downcast_to::<Tick>() {
            Ok(tick) => {
                return self.handle_tick(tick);
            }
            Err(event) => event,
        };

        if let Some(MoveEvent(direction)) = event.downcast_ref::<MoveEvent>() {
            return self.handle_log_movement(*direction);
        }

        if let Some(pointer) = event.downcast_ref::<PointerEvent>()
            && let Some(outcome) = self.handle_pointer_event(pointer)
        {
            return outcome;
        }

        event = match event.downcast_to::<Submit>() {
            Ok(_submit) => {
                self.toggle_log_updates();

                return EventOutcome::Consumed(Vec::new());
            }
            Err(event) => event,
        };

        EventOutcome::Ignored(event)
    }
}

impl Component for LogView {
    fn id(&self) -> ComponentId {
        self.id
    }

    fn kind() -> ComponentKind {
        "content"
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, context: RenderContext<'_>) {
        let block = focused_block(self.title(), context.focused);

        let widget = LogViewWidget::new(&self.logs)
            .block(block)
            .updates_paused(self.log_updates_paused)
            .update_pending(self.log_update.is_some());

        frame.render_stateful_widget(widget, area, &mut self.state);
    }

    fn on(&mut self, event: Box<dyn Event>) -> EventOutcome {
        self.handle_event(event)
    }
}
