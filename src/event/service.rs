use futures::StreamExt;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{debug, error, info};

use crate::event::{TerminalEvent, Tick};

const DEFAULT_TICK_FPS: f64 = 30.0;
const EVENT_BUFFER_SIZE: usize = 64;

#[derive(Debug)]
pub struct TerminalService;

impl TerminalService {
    pub fn spawn() -> (Self, broadcast::Receiver<Result<TerminalEvent, String>>) {
        let (tx, rx) = broadcast::channel(EVENT_BUFFER_SIZE);

        tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();

            loop {
                tokio::select! {
                    _ = tx.closed() => {
                        debug!("terminal service stopped");
                        break;
                    }

                    result = reader.next() => {
                        match result {
                            Some(Ok(event)) => {
                                if tx
                                    .send(Ok(TerminalEvent(event)))
                                    .is_err()
                                {
                                    debug!(
                                        "terminal service has no active receivers"
                                    );
                                    break;
                                }
                            }

                            Some(Err(err)) => {
                                error!(
                                    error = ?err,
                                    "failed to read terminal event"
                                );

                                if tx
                                    .send(Err(err.to_string()))
                                    .is_err()
                                {
                                    break;
                                }
                            }

                            None => {
                                debug!("terminal event stream closed");
                                break;
                            }
                        }
                    }
                }
            }
        });

        (Self, rx)
    }
}

#[derive(Debug)]
pub struct TickService;

impl TickService {
    #[allow(dead_code)]
    pub fn spawn() -> (Self, broadcast::Receiver<Tick>) {
        Self::spawn_with_rate(DEFAULT_TICK_FPS)
    }

    pub fn spawn_with_rate(frames_per_second: f64) -> (Self, broadcast::Receiver<Tick>) {
        assert!(
            frames_per_second.is_finite() && frames_per_second > 0.0,
            "frames_per_second must be finite and greater than zero"
        );

        let tick_rate = Duration::from_secs_f64(1.0 / frames_per_second);
        let (tx, rx) = broadcast::channel(EVENT_BUFFER_SIZE);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);

            // By default, Tokio's first tick completes immediately.
            // Consume it here so the first update occurs after `tick_rate`.
            interval.tick().await;

            loop {
                tokio::select! {
                    _ = tx.closed() => {
                        debug!("tick service stopped");
                        break;
                    }

                    instant = interval.tick() => {
                        let update = Tick (
                            tick_rate,
                        );

                        if tx.send(update).is_err() {
                            info!(
                                ?instant,
                                "tick service has no active receivers"
                            );
                            break;
                        }
                    }
                }
            }
        });

        (Self, rx)
    }
}

impl Default for TickService {
    fn default() -> Self {
        Self
    }
}
