//! Central asynchronous event aggregator.
//!
//! The event queue combines:
//! - terminal events
//! - periodic tick events
//! - application-generated root events
//! - reactions generated while handling another event
//! - updates from any number of background services
//!
//! Every received value is wrapped in an [`EventEnvelope`] before entering
//! the central application event queue.

use crate::event::{
    Event, EventEnvelope, EventMetadata, EventSourceLagged, TerminalError,
    service::{TerminalService, TickService},
};

use color_eyre::eyre::{OptionExt, WrapErr};
use std::future::Future;
use tokio::{
    sync::{broadcast, mpsc},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, warn};

/// Target tick rate in frames per second.
const TICK_FPS: f64 = 5.0;

/// Receives and aggregates all application events.
///
/// This type only transports events. It does not dispatch events to UI
/// components and does not recursively process reactions.
#[derive(Debug)]
pub struct EventQueue {
    sender: mpsc::UnboundedSender<EventEnvelope>,
    receiver: mpsc::UnboundedReceiver<EventEnvelope>,
    attachments: Vec<EventSubscription>,
}

impl EventQueue {
    /// Creates an empty central event queue.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        Self {
            sender,
            receiver,
            attachments: Vec::new(),
        }
    }

    /// Waits for the next event from any registered source.
    pub async fn next(&mut self) -> color_eyre::Result<EventEnvelope> {
        self.receiver
            .recv()
            .await
            .ok_or_eyre("event channel closed unexpectedly")
    }

    /// Sends a concrete root event into the central queue.
    ///
    /// Use this for events originating outside another event's processing,
    /// such as terminal events, ticks, background services, or direct
    /// application actions.
    pub fn send<E>(&self, source: &'static str, event: E) -> color_eyre::Result<()>
    where
        E: Event + 'static,
    {
        self.send_envelope(EventEnvelope::root(source, event))
    }

    /// Sends an already boxed root event into the central queue.
    pub fn send_boxed(
        &self,
        source: &'static str,
        event: Box<dyn Event>,
    ) -> color_eyre::Result<()> {
        self.send_envelope(EventEnvelope::boxed_root(source, event))
    }

    /// Sends an event that was produced as a reaction to another event.
    ///
    /// The parent metadata is used to preserve the causal chain and increment
    /// the reaction depth.
    pub fn send_reaction<E>(
        &self,
        parent: &EventMetadata,
        source: &'static str,
        event: E,
    ) -> color_eyre::Result<()>
    where
        E: Event + 'static,
    {
        self.send_envelope(EventEnvelope::reaction(parent, source, event))
    }

    /// Sends an already boxed reaction event.
    pub fn send_boxed_reaction(
        &self,
        parent: &EventMetadata,
        source: &'static str,
        event: Box<dyn Event>,
    ) -> color_eyre::Result<()> {
        self.send_envelope(EventEnvelope::boxed_reaction(parent, source, event))
    }

    /// Sends an existing envelope into the central queue.
    fn send_envelope(&self, envelope: EventEnvelope) -> color_eyre::Result<()> {
        self.sender
            .send(envelope)
            .wrap_err("failed to send application event")
    }

    /// Keeps an attachment alive for the lifetime of this queue.
    ///
    /// Use this for permanent queue-owned event sources. Sources attached
    /// without calling this method remain controlled by the returned
    /// [`EventAttachment`].
    pub fn retain_attachment(&mut self, attachment: EventSubscription) {
        self.attachments.push(attachment);
    }

    /// Attaches a Tokio broadcast receiver to the central event queue.
    ///
    /// Updates from an attached service are external root events. The mapper
    /// translates the service-specific update into an application event.
    ///
    /// The returned handle controls the lifetime of the forwarding task.
    /// Dropping the handle requests cancellation.
    #[must_use = "the attachment must be retained or it will be cancelled"]
    pub fn attach_broadcast<U, M>(
        &self,
        source: &'static str,
        mut receiver: broadcast::Receiver<U>,
        mapper: M,
    ) -> EventSubscription
    where
        U: Clone + Send + 'static,
        M: Fn(U) -> Box<dyn Event> + Send + Sync + 'static,
    {
        let sender = self.sender.clone();
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();

        let task = tokio::spawn(async move {
            debug!(source, "broadcast event source attached");

            loop {
                tokio::select! {
                    biased;

                    _ = task_cancellation.cancelled() => {
                        debug!(
                            source,
                            "broadcast event source cancellation requested"
                        );

                        break;
                    }

                    _ = sender.closed() => {
                        debug!(
                            source,
                            "central event queue closed"
                        );

                        break;
                    }

                    result = receiver.recv() => {
                        match result {
                            Ok(update) => {
                                let envelope = EventEnvelope::boxed_root(
                                    source,
                                    mapper(update),
                                );

                                if sender.send(envelope).is_err() {
                                    debug!(
                                        source,
                                        "central event receiver was dropped"
                                    );

                                    break;
                                }
                            }

                            Err(
                                broadcast::error::RecvError::Lagged(skipped),
                            ) => {
                                warn!(
                                    source,
                                    skipped,
                                    "event source receiver lagged"
                                );

                                let envelope = EventEnvelope::root(
                                    source,
                                    EventSourceLagged {
                                        source,
                                        skipped,
                                    },
                                );

                                if sender.send(envelope).is_err() {
                                    debug!(
                                        source,
                                        "central event receiver was dropped"
                                    );

                                    break;
                                }
                            }

                            Err(
                                broadcast::error::RecvError::Closed,
                            ) => {
                                debug!(
                                    source,
                                    "broadcast event source closed"
                                );

                                break;
                            }
                        }
                    }
                }
            }

            debug!(source, "broadcast event source detached");
        });

        EventSubscription::new(source, cancellation, task)
    }

    /// Attaches an unbounded MPSC receiver to the central event queue.
    ///
    /// Each incoming update becomes a new root event.
    ///
    /// The returned handle controls the lifetime of the forwarding task.
    /// Dropping the handle requests cancellation.
    #[must_use = "the attachment must be retained or it will be cancelled"]
    pub fn attach_mpsc<U, M>(
        &self,
        source: &'static str,
        mut receiver: mpsc::UnboundedReceiver<U>,
        mapper: M,
    ) -> EventSubscription
    where
        U: Send + 'static,
        M: Fn(U) -> Box<dyn Event> + Send + Sync + 'static,
    {
        let sender = self.sender.clone();
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();

        let task = tokio::spawn(async move {
            debug!(source, "mpsc event source attached");

            loop {
                tokio::select! {
                    biased;

                    _ = task_cancellation.cancelled() => {
                        debug!(
                            source,
                            "mpsc event source cancellation requested"
                        );

                        break;
                    }

                    _ = sender.closed() => {
                        debug!(
                            source,
                            "central event queue closed"
                        );

                        break;
                    }

                    update = receiver.recv() => {
                        let Some(update) = update else {
                            debug!(
                                source,
                                "mpsc event source closed"
                            );

                            break;
                        };

                        let envelope = EventEnvelope::boxed_root(
                            source,
                            mapper(update),
                        );

                        if sender.send(envelope).is_err() {
                            debug!(
                                source,
                                "central event receiver was dropped"
                            );

                            break;
                        }
                    }
                }
            }

            debug!(source, "mpsc event source detached");
        });

        EventSubscription::new(source, cancellation, task)
    }

    /// Attaches an arbitrary asynchronous event producer.
    ///
    /// The producer receives an [`EventProducer`] instead of the raw channel.
    /// This ensures events originating from the task are correctly wrapped
    /// with source information.
    ///
    /// The task should observe [`EventProducer::cancelled`] when it performs
    /// long-running or repeated work.
    ///
    /// The returned handle controls the lifetime of the producer task.
    #[must_use = "the attachment must be retained or it will be cancelled"]
    pub fn attach_task<F, Fut>(&self, source: &'static str, task: F) -> EventSubscription
    where
        F: FnOnce(EventProducer) -> Fut + Send + 'static,
        Fut: Future<Output = color_eyre::Result<()>> + Send + 'static,
    {
        let cancellation = CancellationToken::new();

        let producer = EventProducer {
            source,
            sender: self.sender.clone(),
            cancellation: cancellation.clone(),
        };

        let task_cancellation = cancellation.clone();

        let task_handle = tokio::spawn(async move {
            debug!(source, "custom event task attached");

            tokio::select! {
                biased;

                _ = task_cancellation.cancelled() => {
                    debug!(
                        source,
                        "custom event source cancellation requested"
                    );
                }

                result = task(producer) => {
                    if let Err(error) = result {
                        error!(
                            source,
                            error = ?error,
                            "custom event source terminated with an error"
                        );
                    }
                }
            }

            debug!(source, "custom event source detached");
        });

        EventSubscription::new(source, cancellation, task_handle)
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        let mut events = Self::new();

        let (_tick_service, tick_updates) = TickService::spawn_with_rate(TICK_FPS);

        let tick_attachment =
            events.attach_broadcast("tick", tick_updates, |update| update.boxed());

        events.retain_attachment(tick_attachment);

        let (_terminal_service, terminal_updates) = TerminalService::spawn();

        let terminal_attachment =
            events.attach_broadcast("terminal", terminal_updates, |update| match update {
                Ok(event) => event.boxed(),
                Err(message) => TerminalError(message).boxed(),
            });

        events.retain_attachment(terminal_attachment);

        events
    }
}

/// Controls the lifetime of an event source attached to the event queue.
///
/// Calling [`EventSubscription::detach`] requests graceful cancellation and
/// waits until the forwarding task has terminated.
///
/// Dropping the handle requests cancellation, but does not wait for the task
/// to finish.
#[derive(Debug)]
pub struct EventSubscription {
    source: &'static str,
    cancellation: CancellationToken,
    task: Option<JoinHandle<()>>,
}

impl EventSubscription {
    fn new(source: &'static str, cancellation: CancellationToken, task: JoinHandle<()>) -> Self {
        Self {
            source,
            cancellation,
            task: Some(task),
        }
    }

    /// Returns the name of the attached event source.
    pub fn source(&self) -> &'static str {
        self.source
    }

    /// Returns whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// Requests cancellation without waiting for task termination.
    pub fn cancel(&self) {
        debug!(source = self.source, "cancelling event source attachment");

        self.cancellation.cancel();
    }

    /// Requests cancellation and waits for the task to terminate.
    pub async fn detach(mut self) -> color_eyre::Result<()> {
        debug!(source = self.source, "detaching event source");

        self.cancellation.cancel();

        if let Some(task) = self.task.take() {
            task.await.wrap_err_with(|| {
                format!("event source task {:?} failed while detaching", self.source,)
            })?;
        }

        debug!(source = self.source, "event source detached");

        Ok(())
    }

    /// Immediately aborts the attached task.
    ///
    /// Prefer [`EventAttachment::detach`] for ordinary shutdown.
    pub fn abort(mut self) {
        warn!(source = self.source, "aborting event source attachment");

        self.cancellation.cancel();

        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl Drop for EventSubscription {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

/// Restricted sender passed to attached asynchronous producers.
///
/// This preserves the producer's source name and ensures that ordinary task
/// output enters the queue as root events.
#[derive(Clone, Debug)]
pub struct EventProducer {
    source: &'static str,
    sender: mpsc::UnboundedSender<EventEnvelope>,
    cancellation: CancellationToken,
}

impl EventProducer {
    /// Returns the name of this event producer.
    pub fn source(&self) -> &'static str {
        self.source
    }

    /// Returns whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// Waits until cancellation is requested.
    pub async fn cancelled(&self) {
        self.cancellation.cancelled().await;
    }

    /// Sends a concrete root event from this producer.
    pub fn send<E>(&self, event: E) -> color_eyre::Result<()>
    where
        E: Event + 'static,
    {
        self.send_boxed(event.boxed())
    }

    /// Sends a boxed root event from this producer.
    pub fn send_boxed(&self, event: Box<dyn Event>) -> color_eyre::Result<()> {
        self.sender
            .send(EventEnvelope::boxed_root(self.source, event))
            .wrap_err_with(|| format!("failed to send event from source {:?}", self.source,))
    }

    /// Sends a reaction from this producer.
    ///
    /// This is useful if asynchronous work was started in response to another
    /// event and the causal relationship should be retained.
    pub fn send_reaction<E>(&self, parent: &EventMetadata, event: E) -> color_eyre::Result<()>
    where
        E: Event + 'static,
    {
        self.send_boxed_reaction(parent, event.boxed())
    }

    /// Sends a boxed reaction from this producer.
    pub fn send_boxed_reaction(
        &self,
        parent: &EventMetadata,
        event: Box<dyn Event>,
    ) -> color_eyre::Result<()> {
        self.sender
            .send(EventEnvelope::boxed_reaction(parent, self.source, event))
            .wrap_err_with(|| format!("failed to send reaction from source {:?}", self.source,))
    }
}
