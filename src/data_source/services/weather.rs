use std::marker::PhantomData;

use chrono::{DateTime, Duration, FixedOffset, Utc};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, instrument};

use crate::data_source::end_points::brightsky::{
    BrightSky, Location, Metric, TimeSeries, UnitSystem, WeatherQuery,
};

/// Configuration for a rolling weather-data service.
///
/// The service fetches data from a rolling window ending at the current
/// timestamp:
///
/// ```text
/// window_start = now - timespan
/// window_end   = now
/// ```
///
/// After the initial update, the service uses its cursor and overlap to avoid
/// downloading the complete window on every trigger.
#[derive(Clone, Debug)]
pub struct WeatherServiceConfig {
    /// Location from which weather data is fetched.
    location: Location,

    /// Timezone used by Bright Sky for parsing and returning timestamps.
    timezone: String,

    /// Unit system requested from Bright Sky.
    units: UnitSystem,

    /// Size of the rolling time window ending at the current timestamp.
    timespan: Duration,

    /// Amount of previously seen data fetched again during each update.
    ///
    /// Weather observations can be corrected after their initial publication.
    /// Consumers should therefore upsert records by timestamp rather than
    /// blindly appending all returned records.
    overlap: Duration,

    /// Maximum distance, in metres, within which Bright Sky may select a
    /// weather source.
    ///
    /// This option is only valid for coordinate-based locations.
    max_distance_m: Option<u32>,
}

impl WeatherServiceConfig {
    /// Creates a service configuration.
    ///
    /// The default configuration uses:
    ///
    /// - UTC timestamps
    /// - DWD civil and meteorological units
    /// - two hours of overlap
    /// - no explicit maximum station distance
    pub fn new(location: Location, timespan: Duration) -> Self {
        Self {
            location,
            timezone: "UTC".to_owned(),
            units: UnitSystem::Dwd,
            timespan,
            overlap: Duration::hours(2),
            max_distance_m: None,
        }
    }

    pub fn location(&self) -> &Location {
        &self.location
    }

    pub fn timezone_name(&self) -> &str {
        &self.timezone
    }

    pub fn unit_system(&self) -> UnitSystem {
        self.units
    }

    pub fn timespan_value(&self) -> Duration {
        self.timespan
    }

    pub fn overlap_value(&self) -> Duration {
        self.overlap
    }

    pub fn max_distance_m(&self) -> Option<u32> {
        self.max_distance_m
    }

    pub fn timezone(mut self, timezone: impl Into<String>) -> Self {
        self.timezone = timezone.into();
        self
    }

    pub fn units(mut self, units: UnitSystem) -> Self {
        self.units = units;
        self
    }

    pub fn timespan(mut self, timespan: Duration) -> Self {
        self.timespan = timespan;
        self
    }

    pub fn overlap(mut self, overlap: Duration) -> Self {
        self.overlap = overlap;
        self
    }

    pub fn max_distance(mut self, metres: u32) -> Self {
        self.max_distance_m = Some(metres);
        self
    }

    pub fn without_max_distance(mut self) -> Self {
        self.max_distance_m = None;
        self
    }

    fn validate(&self) -> Result<(), WeatherServiceError> {
        if self.timezone.trim().is_empty() {
            return Err(WeatherServiceError::InvalidConfiguration(
                "timezone must not be empty".into(),
            ));
        }

        if self.timespan <= Duration::zero() {
            return Err(WeatherServiceError::InvalidConfiguration(
                "timespan must be positive".into(),
            ));
        }

        if self.overlap < Duration::zero() {
            return Err(WeatherServiceError::InvalidConfiguration(
                "overlap must not be negative".into(),
            ));
        }

        if self.overlap > self.timespan {
            return Err(WeatherServiceError::InvalidConfiguration(
                "overlap must not exceed timespan".into(),
            ));
        }

        if let Some(distance) = self.max_distance_m {
            if distance > 500_000 {
                return Err(WeatherServiceError::InvalidConfiguration(
                    "maximum source distance must not exceed 500,000 metres".into(),
                ));
            }

            if !matches!(self.location, Location::Coordinates { .. }) {
                return Err(WeatherServiceError::InvalidConfiguration(
                    "maximum source distance is only valid for coordinate locations".into(),
                ));
            }
        }

        Ok(())
    }
}

/// Mutable state maintained by the weather service.
#[derive(Clone, Debug, Default)]
struct WeatherServiceState {
    /// Latest weather-record timestamp observed during a successful update.
    last_seen: Option<DateTime<FixedOffset>>,

    /// Time at which the most recent successful update completed.
    last_updated_at: Option<DateTime<FixedOffset>>,
}

/// A configurable, manually triggered weather service.
///
/// The metric type determines the value type in the returned data:
///
/// ```ignore
/// WeatherService<Temperature>
/// WeatherService<PressureMsl>
/// WeatherService<Condition>
/// ```
///
/// A service instance stores an incremental cursor. The first update requests
/// the complete rolling timespan. Subsequent updates request only the range
/// beginning at:
///
/// ```text
/// max(now - timespan, last_seen - overlap)
/// ```
pub struct WeatherService<M>
where
    M: Metric,
{
    client: BrightSky,
    config: WeatherServiceConfig,

    /// Ensures that only one update runs at a time.
    update_lock: Mutex<()>,

    /// Allows inspection of service state without waiting for an active
    /// network request to complete.
    state: RwLock<WeatherServiceState>,

    marker: PhantomData<fn() -> M>,
}

impl<M> WeatherService<M>
where
    M: Metric,
{
    /// Creates a weather service using an existing Bright Sky client.
    pub fn new(
        client: BrightSky,
        config: WeatherServiceConfig,
    ) -> Result<Self, WeatherServiceError> {
        config.validate()?;

        Ok(Self {
            client,
            config,
            update_lock: Mutex::new(()),
            state: RwLock::new(WeatherServiceState::default()),
            marker: PhantomData,
        })
    }

    /// Creates a weather service using the default public Bright Sky client.
    pub fn with_default_client(config: WeatherServiceConfig) -> Result<Self, WeatherServiceError> {
        let client = BrightSky::new()?;
        Self::new(client, config)
    }

    pub fn config(&self) -> &WeatherServiceConfig {
        &self.config
    }

    /// Returns the latest weather timestamp observed by this service.
    pub async fn last_seen(&self) -> Option<DateTime<FixedOffset>> {
        self.state.read().await.last_seen
    }

    /// Returns the completion time of the most recent successful update.
    pub async fn last_updated_at(&self) -> Option<DateTime<FixedOffset>> {
        self.state.read().await.last_updated_at
    }

    /// Returns a snapshot of the current service status.
    pub async fn status(&self) -> WeatherServiceStatus {
        let state = self.state.read().await;

        WeatherServiceStatus {
            metric: M::NAME,
            last_seen: state.last_seen,
            last_updated_at: state.last_updated_at,
        }
    }

    /// Resets the incremental cursor.
    ///
    /// The next call to `fetch_update` requests the complete current rolling
    /// window.
    pub async fn reset(&self) {
        let _update_guard = self.update_lock.lock().await;

        let mut state = self.state.write().await;
        *state = WeatherServiceState::default();

        debug!(metric = M::NAME, "reset weather service cursor");
    }

    /// Fetches the newest available weather update.
    ///
    /// The first invocation requests:
    ///
    /// ```text
    /// now - timespan .. now
    /// ```
    ///
    /// Later invocations request:
    ///
    /// ```text
    /// max(now - timespan, last_seen - overlap) .. now
    /// ```
    ///
    /// The cursor is advanced only after a successful request. Failed requests
    /// leave the existing cursor unchanged.
    ///
    /// Only one update may run for a service instance at a time. Concurrent
    /// callers wait for the active update and then calculate their request
    /// from the newly updated cursor.
    #[instrument(
        name = "weather_service.fetch_update",
        skip(self),
        fields(
            metric = M::NAME,
            location = %self.config.location,
            unit_system = %self.config.units,
            window_start = tracing::field::Empty,
            request_start = tracing::field::Empty,
            request_end = tracing::field::Empty,
            previous_cursor = tracing::field::Empty,
            new_cursor = tracing::field::Empty,
            records = tracing::field::Empty
        )
    )]
    pub async fn fetch_update(&self) -> Result<WeatherUpdate<M>, WeatherServiceError> {
        let _update_guard = self.update_lock.lock().await;

        let requested_at = now();
        let window_start = requested_at - self.config.timespan;

        let previous_cursor = {
            let state = self.state.read().await;
            state.last_seen
        };

        let request_start = previous_cursor
            .map(|cursor| cursor - self.config.overlap)
            .map(|cursor_start| cursor_start.max(window_start))
            .unwrap_or(window_start);

        let span = tracing::Span::current();

        span.record("window_start", tracing::field::display(window_start));

        span.record("request_start", tracing::field::display(request_start));

        span.record("request_end", tracing::field::display(requested_at));

        if let Some(cursor) = previous_cursor {
            span.record("previous_cursor", tracing::field::display(cursor));
        }

        debug!(
            metric = M::NAME,
            %window_start,
            %request_start,
            request_end = %requested_at,
            previous_cursor = ?previous_cursor,
            "fetching weather update"
        );

        let mut query = WeatherQuery::new(self.config.location.clone(), request_start)
            .until(requested_at)
            .timezone(self.config.timezone.clone())
            .units(self.config.units);

        if let Some(max_distance_m) = self.config.max_distance_m {
            query = query.max_distance(max_distance_m);
        }

        let data = self.client.weather(&query).await?;
        let series = data.series::<M>();

        let new_cursor = series
            .last()
            .map(|record| record.timestamp)
            .or(previous_cursor);

        let completed_at = now();

        {
            let mut state = self.state.write().await;

            state.last_seen = new_cursor;
            state.last_updated_at = Some(completed_at);
        }

        span.record("records", series.len() as u64);

        if let Some(cursor) = new_cursor {
            span.record("new_cursor", tracing::field::display(cursor));
        }

        if data.is_empty() {
            debug!(
                metric = M::NAME,
                %request_start,
                request_end = %requested_at,
                "weather update contained no measurements"
            );
        } else {
            info!(
                metric = M::NAME,
                records = series.len(),
                cursor = ?new_cursor,
                "weather update completed"
            );
        }

        Ok(WeatherUpdate {
            data: series,
            requested_at,
            completed_at,
            window_start,
            range_start: request_start,
            range_end: requested_at,
            previous_cursor,
            new_cursor,
        })
    }
}

/// Metadata and typed weather data returned by one update.
#[derive(Debug, Clone)]
pub struct WeatherUpdate<M>
where
    M: Metric,
{
    /// Homogeneous typed weather records returned by Bright Sky.
    pub data: TimeSeries<M>,

    /// Time at which the update began.
    pub requested_at: DateTime<FixedOffset>,

    /// Time at which the update completed successfully.
    pub completed_at: DateTime<FixedOffset>,

    /// Beginning of the configured rolling time window.
    pub window_start: DateTime<FixedOffset>,

    /// Actual beginning of the HTTP request range.
    ///
    /// This may be later than `window_start` when an incremental cursor is
    /// available.
    pub range_start: DateTime<FixedOffset>,

    /// End of the HTTP request range.
    pub range_end: DateTime<FixedOffset>,

    /// Cursor value before the update.
    pub previous_cursor: Option<DateTime<FixedOffset>>,

    /// Latest known weather timestamp after the update.
    pub new_cursor: Option<DateTime<FixedOffset>>,
}

impl<M> WeatherUpdate<M>
where
    M: Metric,
{
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns `true` when this update was based on an existing cursor.
    ///
    /// Such an update normally includes the configured overlap.
    pub fn contains_overlap(&self) -> bool {
        self.previous_cursor.is_some()
    }
}

/// Read-only snapshot of a weather service's state.
#[derive(Clone, Debug)]
pub struct WeatherServiceStatus {
    pub metric: &'static str,
    pub last_seen: Option<DateTime<FixedOffset>>,
    pub last_updated_at: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Error)]
pub enum WeatherServiceError {
    #[error("invalid weather service configuration: {0}")]
    InvalidConfiguration(String),

    #[error(transparent)]
    BrightSky(#[from] crate::data_source::end_points::brightsky::Error),
}

fn now() -> DateTime<FixedOffset> {
    Utc::now().fixed_offset()
}
