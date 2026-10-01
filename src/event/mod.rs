mod event_handler;
pub(crate) mod service;

use crossterm::event::Event as CrosstermEvent;
pub use event_handler::*;
use std::any::Any;
use std::collections::VecDeque;
use std::fmt::{Debug, Display};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tracing::{debug, error, trace, warn};

static NEXT_EVENT_ID: AtomicU64 = AtomicU64::new(1);

/// Trait implemented by every component-routable event.
///
/// Use the [`new_event!`] macro to implement this trait for concrete event
/// types.
pub trait Event: Any + Send + Sync + Debug {
    /// Human-readable event type or variant name.
    fn event_name(&self) -> &'static str;

    /// Clones this event into a new trait-object allocation.
    ///
    /// Prefer borrowing or owned dispatch where possible. This remains
    /// available for the cases where cloning is intentional.
    fn box_clone(&self) -> Box<dyn Event>;

    /// Returns this event as [`Any`] for borrowed downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Converts this event into an owned [`Any`] trait object.
    ///
    /// This enables moving payloads such as `Vec<T>` out of an event without
    /// cloning them.
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync>;

    fn boxed(self) -> Box<dyn Event>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}

impl dyn Event {
    /// Attempts to borrow this event as `T`.
    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: Event + 'static,
    {
        self.as_any().downcast_ref::<T>()
    }

    /// Returns whether this event has concrete type `T`.
    pub fn is<T>(&self) -> bool
    where
        T: Event + 'static,
    {
        self.as_any().is::<T>()
    }

    /// Attempts to convert this boxed event into its concrete type.
    ///
    /// On success, ownership of the concrete event is returned. Payloads
    /// contained by the event can then be moved out without cloning.
    ///
    /// On failure, the original boxed event is returned unchanged.
    pub fn downcast_to<T>(self: Box<Self>) -> Result<T, Box<Self>>
    where
        T: Event + 'static,
    {
        if !self.as_any().is::<T>() {
            return Err(self);
        }

        match self.into_any().downcast::<T>() {
            Ok(event) => Ok(*event),
            Err(_) => {
                unreachable!("event passed is::<T>(), but the subsequent downcast failed")
            }
        }
    }
}

impl Clone for Box<dyn Event> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

/// Generates concrete event types and implements [`Event`] for them.
///
/// # Forms
///
/// ```rust,ignore
/// new_event!(Quit);
///
/// new_event!(Tick(Duration));
///
/// new_event!(Position(u16, u16));
///
/// new_event!(Resize {
///     width: u16,
///     height: u16,
/// });
///
///
/// // The explicit `enum` form remains supported:
/// new_event!(enum ScrollEvent {
///     Up,
///     Down,
///     To(u16),
///     Position { x: u16, y: u16 },
/// });
/// ```
#[macro_export]
macro_rules! new_event {
    // -------------------------------------------------------------------------
    // Unit struct
    //
    // new_event!(Quit);
    // -------------------------------------------------------------------------
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name;

        $crate::new_event!(@impl_event $name);
    };

    // -------------------------------------------------------------------------
    // Generic tuple struct
    //
    // new_event!(Wrapped<E>);
    // new_event!(Pair<A, B>);
    //
    // Each generic argument becomes one tuple field.
    // -------------------------------------------------------------------------
    ($name:ident<$($parameter:ident),+ $(,)?>) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name<$($parameter),+>(
            $(pub $parameter),+
        )
        where
            $(
                $parameter:
                    $crate::event::Event
                    + Clone
                    + 'static
            ),+;

        impl<$($parameter),+> $crate::event::Event
            for $name<$($parameter),+>
        where
            $(
                $parameter:
                    $crate::event::Event
                    + Clone
                    + 'static
            ),+
        {
            fn event_name(&self) -> &'static str {
                stringify!($name)
            }

            fn box_clone(&self) -> Box<dyn $crate::event::Event> {
                Box::new(self.clone())
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }

            fn into_any(
                self: Box<Self>,
            ) -> Box<
                dyn ::std::any::Any
                    + ::core::marker::Send
                    + ::core::marker::Sync
            > {
                self
            }
        }
    };

    // -------------------------------------------------------------------------
    // Tuple struct
    //
    // new_event!(Tick(Duration));
    // new_event!(Position(u16, u16));
    // -------------------------------------------------------------------------
    ($name:ident($($parameter:ty),+ $(,)?)) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name($(pub $parameter),+);

        $crate::new_event!(@impl_event $name);
    };

    // -------------------------------------------------------------------------
    // Fat enum
    //
    // This arm must appear before the named-field struct arm.
    //
    // -------------------------------------------------------------------------
    (
        $name:ident {
            $(
                $variant:ident
                $(($($tuple_type:ty),+ $(,)?))?
                $({
                    $($field:ident : $field_type:ty),+ $(,)?
                })?
            ),+ $(,)?
        }
    ) => {
        $crate::new_event! {
            @enum
            $name {
                $(
                    $variant
                    $(($($tuple_type),+))?
                    $({
                        $($field: $field_type),+
                    })?
                ),+
            }
        }
    };

    // -------------------------------------------------------------------------
    // Named-field struct
    //
    // new_event!(Resize {
    //     width: u16,
    //     height: u16,
    // });
    // -------------------------------------------------------------------------
    (
        $name:ident {
            $($field:ident : $ty:ty),+ $(,)?
        }
    ) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name {
            $(pub $field: $ty),+
        }

        $crate::new_event!(@impl_event $name);
    };

    // -------------------------------------------------------------------------
    // Explicit enum syntax
    //
    // Kept for backward compatibility.
    // -------------------------------------------------------------------------
    (
        enum $name:ident {
            $(
                $variant:ident
                $(($($tuple_type:ty),+ $(,)?))?
                $({
                    $($field:ident : $field_type:ty),+ $(,)?
                })?
            ),+ $(,)?
        }
    ) => {
        $crate::new_event! {
            @enum
            $name {
                $(
                    $variant
                    $(($($tuple_type),+))?
                    $({
                        $($field: $field_type),+
                    })?
                ),+
            }
        }
    };

    // -------------------------------------------------------------------------
    // Internal enum implementation
    // -------------------------------------------------------------------------
    (
        @enum
        $name:ident {
            $(
                $variant:ident
                $(($($tuple_type:ty),+))?
                $({
                    $($field:ident : $field_type:ty),+
                })?
            ),+
        }
    ) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $name {
            $(
                $variant
                $(($($tuple_type),+))?
                $({
                    $($field: $field_type),+
                })?
            ),+
        }

        impl $crate::event::Event for $name {
            fn event_name(&self) -> &'static str {
                match self {
                    $(
                        Self::$variant
                        $(( $crate::new_event!(@tuple_pattern $($tuple_type),+) ))?
                        $({ $($field: _),+ })?
                        => concat!(
                            stringify!($name),
                            "::",
                            stringify!($variant)
                        ),
                    )+
                }
            }

            fn box_clone(&self) -> Box<dyn $crate::event::Event> {
                Box::new(self.clone())
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }

            fn into_any(
                self: Box<Self>,
            ) -> Box<
                dyn ::std::any::Any
                    + ::core::marker::Send
                    + ::core::marker::Sync
            > {
                self
            }
        }
    };

    // -------------------------------------------------------------------------
    // Internal Event implementation for non-generic structs
    // -------------------------------------------------------------------------
    (@impl_event $name:ident) => {
        impl $crate::event::Event for $name {
            fn event_name(&self) -> &'static str {
                stringify!($name)
            }

            fn box_clone(&self) -> Box<dyn $crate::event::Event> {
                Box::new(self.clone())
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }

            fn into_any(
                self: Box<Self>,
            ) -> Box<
                dyn ::std::any::Any
                    + ::core::marker::Send
                    + ::core::marker::Sync
            > {
                self
            }
        }
    };

    // Produces one wildcard for each tuple-variant field.
    (@tuple_pattern $first:ty $(, $remaining:ty)*) => {
        _ $(, $crate::new_event!(@ignore_type $remaining))*
    };

    (@ignore_type $value:ty) => {
        _
    };
}

/// Unique identifier for one dispatched event instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EventId(u64);

impl EventId {
    fn next() -> Self {
        Self(NEXT_EVENT_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Causality and diagnostic information associated with an event.
///
/// This is separate from the event payload so it remains available after a
/// handler takes ownership of the concrete event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventMetadata {
    /// Identifier of this event instance.
    id: EventId,

    /// Identifier of the first event in this causal chain.
    root_id: EventId,

    /// Identifier of the event that directly caused this event.
    parent_id: Option<EventId>,

    /// Number of reaction steps from the root event.
    depth: usize,

    /// Event source or handler that emitted this event.
    source: &'static str,
}

impl EventMetadata {
    pub fn id(&self) -> EventId {
        self.id
    }

    pub fn root_id(&self) -> EventId {
        self.root_id
    }

    pub fn parent_id(&self) -> Option<EventId> {
        self.parent_id
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn source(&self) -> &'static str {
        self.source
    }
}

/// An event payload together with its transport and causality metadata.
#[derive(Debug)]
pub struct EventEnvelope {
    pub metadata: EventMetadata,
    pub event: Box<dyn Event>,
}

impl EventEnvelope {
    /// Creates a new externally-originated root event.
    pub(super) fn root(source: &'static str, event: impl Event + 'static) -> Self {
        Self::boxed_root(source, event.boxed())
    }

    /// Creates a new externally-originated boxed root event.
    pub(super) fn boxed_root(source: &'static str, event: Box<dyn Event>) -> Self {
        let id = EventId::next();

        Self {
            metadata: EventMetadata {
                id,
                root_id: id,
                parent_id: None,
                depth: 0,
                source,
            },
            event,
        }
    }

    /// Creates an event caused by another event.
    pub(super) fn reaction(
        parent: &EventMetadata,
        source: &'static str,
        event: impl Event + 'static,
    ) -> Self {
        Self::boxed_reaction(parent, source, event.boxed())
    }

    /// Creates a boxed event caused by another event.
    pub(super) fn boxed_reaction(
        parent: &EventMetadata,
        source: &'static str,
        event: Box<dyn Event>,
    ) -> Self {
        Self {
            metadata: EventMetadata {
                id: EventId::next(),
                root_id: parent.root_id,
                parent_id: Some(parent.id),
                depth: parent.depth + 1,
                source,
            },
            event,
        }
    }

    pub fn event_name(&self) -> &'static str {
        self.event.event_name()
    }

    /// Tries to borrow the event payload as `T`.
    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: Event + 'static,
    {
        self.event.downcast_ref::<T>()
    }

    /// Returns whether this envelope contains an event of concrete type `T`.
    pub fn is<T>(&self) -> bool
    where
        T: Event + 'static,
    {
        self.event.is::<T>()
    }

    /// Tries to consume this envelope and return the concrete event.
    ///
    /// On success, both the metadata and concrete event are returned. The
    /// event's fields can be moved out without cloning.
    ///
    /// On failure, the original envelope is reconstructed and returned.
    pub fn try_downcast<T>(self) -> Result<OwnedEvent<T>, Self>
    where
        T: Event + 'static,
    {
        let Self { metadata, event } = self;

        match event.downcast_to::<T>() {
            Ok(event) => Ok(OwnedEvent { metadata, event }),
            Err(event) => Err(Self { metadata, event }),
        }
    }

    /// Splits the envelope into metadata and payload.
    pub fn into_parts(self) -> (EventMetadata, Box<dyn Event>) {
        (self.metadata, self.event)
    }

    /// Reconstructs an envelope from metadata and payload.
    pub fn from_parts(metadata: EventMetadata, event: Box<dyn Event>) -> Self {
        Self { metadata, event }
    }
}

/// Result of successfully consuming and downcasting an envelope.
#[derive(Debug)]
pub struct OwnedEvent<T> {
    pub metadata: EventMetadata,
    pub event: T,
}

impl<T> OwnedEvent<T> {
    pub fn into_parts(self) -> (EventMetadata, T) {
        (self.metadata, self.event)
    }
}

impl<T: Debug> Display for OwnedEvent<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OwnedEvent: \n\t{:?}\n\t{:?}", self.metadata, self.event)
    }
}

/// Result returned by an [`EventHandler`].
pub enum EventOutcome {
    /// Continue passing the original event to subsequent handlers.
    Continue {
        envelope: EventEnvelope,
        reactions: Vec<Box<dyn Event>>,
    },

    /// The handler consumed the event payload.
    ///
    /// Subsequent handlers cannot receive the original event, but the
    /// dispatcher can still use `cause` to construct reaction envelopes.
    Consumed {
        cause: EventMetadata,
        reactions: Vec<Box<dyn Event>>,
    },
}

impl EventOutcome {
    /// Indicates that the event was neither consumed nor reacted to.
    pub fn ignored(envelope: EventEnvelope) -> Self {
        Self::Continue {
            envelope,
            reactions: Vec::new(),
        }
    }

    /// Returns the event for subsequent handlers and emits reactions.
    pub fn observed(envelope: EventEnvelope, reactions: Vec<Box<dyn Event>>) -> Self {
        Self::Continue {
            envelope,
            reactions,
        }
    }

    /// Indicates that the handler consumed the event.
    pub fn consumed(cause: EventMetadata, reactions: Vec<Box<dyn Event>>) -> Self {
        Self::Consumed { cause, reactions }
    }
}

/// A component capable of receiving events.
///
/// Events pass through handlers in registration order. A handler may inspect
/// and return an event, or consume it and stop further propagation.
pub trait EventHandler {
    fn handler_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn handle_event(&mut self, envelope: EventEnvelope) -> EventOutcome;
}

/// Queue-based event dispatcher with causality tracking and loop protection.
///
/// Reactions are queued rather than dispatched recursively.
pub struct EventDispatcher {
    queue: VecDeque<EventEnvelope>,
    max_depth: usize,
    max_events_per_drain: usize,
    max_queue_length: usize,
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            max_depth: 64,
            max_events_per_drain: 10_000,
            max_queue_length: 10_000,
        }
    }

    pub fn with_limits(
        max_depth: usize,
        max_events_per_drain: usize,
        max_queue_length: usize,
    ) -> Self {
        assert!(max_depth > 0, "max_depth must be greater than zero");
        assert!(
            max_events_per_drain > 0,
            "max_events_per_drain must be greater than zero"
        );
        assert!(
            max_queue_length > 0,
            "max_queue_length must be greater than zero"
        );

        Self {
            queue: VecDeque::new(),
            max_depth,
            max_events_per_drain,
            max_queue_length,
        }
    }

    pub fn queue_length(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Queues a concrete root event.
    ///
    /// Returns `false` if the queue limit has been reached.
    pub fn push(&mut self, source: &'static str, event: impl Event + 'static) -> bool {
        self.push_envelope(EventEnvelope::root(source, event))
    }

    /// Queues a boxed root event.
    ///
    /// Returns `false` if the queue limit has been reached.
    pub fn push_boxed(&mut self, source: &'static str, event: Box<dyn Event>) -> bool {
        self.push_envelope(EventEnvelope::boxed_root(source, event))
    }

    /// Processes queued events until the queue is empty or the processing
    /// budget is exhausted.
    pub fn drain(&mut self, handlers: &mut [Box<dyn EventHandler>]) {
        let mut processed = 0usize;

        while let Some(envelope) = self.queue.pop_front() {
            if processed >= self.max_events_per_drain {
                error!(
                    processed,
                    queued = self.queue.len() + 1,
                    max_events_per_drain = self.max_events_per_drain,
                    "event processing budget exceeded; clearing event queue"
                );

                self.queue.clear();
                return;
            }

            processed += 1;

            if envelope.metadata.depth > self.max_depth {
                let root_id = envelope.metadata.root_id;

                error!(
                    event = envelope.event_name(),
                    event_id = envelope.metadata.id.get(),
                    parent_id = envelope.metadata.parent_id.map(EventId::get),
                    root_id = root_id.get(),
                    depth = envelope.metadata.depth,
                    max_depth = self.max_depth,
                    source = envelope.metadata.source,
                    "event reaction depth exceeded"
                );

                self.discard_root(root_id);
                continue;
            }

            self.dispatch_one(envelope, handlers);
        }
    }

    fn dispatch_one(
        &mut self,
        mut envelope: EventEnvelope,
        handlers: &mut [Box<dyn EventHandler>],
    ) {
        debug!(
            event = envelope.event_name(),
            event_id = envelope.metadata.id.get(),
            parent_id = envelope.metadata.parent_id.map(EventId::get),
            root_id = envelope.metadata.root_id.get(),
            depth = envelope.metadata.depth,
            source = envelope.metadata.source,
            "dispatching event"
        );

        for handler in handlers.iter_mut() {
            let handler_name = handler.handler_name();

            trace!(
                event = envelope.event_name(),
                event_id = envelope.metadata.id.get(),
                handler = handler_name,
                "calling event handler"
            );

            match handler.handle_event(envelope) {
                EventOutcome::Continue {
                    envelope: returned,
                    reactions,
                } => {
                    self.enqueue_reactions(&returned.metadata, handler_name, reactions);

                    envelope = returned;
                }

                EventOutcome::Consumed { cause, reactions } => {
                    debug!(
                        event_id = cause.id.get(),
                        root_id = cause.root_id.get(),
                        handler = handler_name,
                        "event consumed"
                    );

                    self.enqueue_reactions(&cause, handler_name, reactions);

                    return;
                }
            }
        }

        trace!(
            event = envelope.event_name(),
            event_id = envelope.metadata.id.get(),
            root_id = envelope.metadata.root_id.get(),
            "event reached end of handler chain"
        );
    }

    fn enqueue_reactions(
        &mut self,
        cause: &EventMetadata,
        source: &'static str,
        reactions: Vec<Box<dyn Event>>,
    ) {
        for reaction in reactions {
            let reaction_name = reaction.event_name();
            let next_depth = cause.depth + 1;

            if next_depth > self.max_depth {
                error!(
                    reaction = reaction_name,
                    parent_id = cause.id.get(),
                    root_id = cause.root_id.get(),
                    next_depth,
                    max_depth = self.max_depth,
                    source,
                    "refusing reaction because depth limit would be exceeded"
                );

                self.discard_root(cause.root_id);
                continue;
            }

            let envelope = EventEnvelope::boxed_reaction(cause, source, reaction);

            debug!(
                reaction = envelope.event_name(),
                event_id = envelope.metadata.id.get(),
                parent_id = envelope.metadata.parent_id.map(EventId::get),
                root_id = envelope.metadata.root_id.get(),
                depth = envelope.metadata.depth,
                source,
                "queueing event reaction"
            );

            self.push_envelope(envelope);
        }
    }

    fn push_envelope(&mut self, envelope: EventEnvelope) -> bool {
        if self.queue.len() >= self.max_queue_length {
            error!(
                queue_length = self.queue.len(),
                max_queue_length = self.max_queue_length,
                event = envelope.event_name(),
                event_id = envelope.metadata.id.get(),
                root_id = envelope.metadata.root_id.get(),
                "event queue limit exceeded; dropping event"
            );

            return false;
        }

        self.queue.push_back(envelope);
        true
    }

    fn discard_root(&mut self, root_id: EventId) {
        let previous_length = self.queue.len();

        self.queue.retain(|event| event.metadata.root_id != root_id);

        let discarded = previous_length - self.queue.len();

        warn!(
            root_id = root_id.get(),
            discarded, "discarded pending events from causal chain"
        );
    }
}

// Standard application events.

new_event!(Quit);
new_event!(Tick(Duration));
new_event!(TerminalEvent(CrosstermEvent));
new_event!(TerminalError(String));
new_event!(EventSourceLagged {
    source: &'static str,
    skipped: u64,
});
