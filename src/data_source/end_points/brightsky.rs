use std::{collections::HashMap, fmt, marker::PhantomData, time::Duration};

use chrono::{DateTime, FixedOffset};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, instrument, warn};

const DEFAULT_BASE_URL: &str = "https://api.brightsky.dev";

/// An asynchronous client for the Bright Sky weather API.
#[derive(Clone, Debug)]
pub struct BrightSky {
    http: Client,
    base_url: String,
}

impl BrightSky {
    /// Creates a client for the public Bright Sky instance.
    pub fn new() -> Result<Self, Error> {
        Self::with_base_url(DEFAULT_BASE_URL)
    }

    /// Creates a client for a custom or self-hosted Bright Sky instance.
    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self, Error> {
        let base_url = base_url.into().trim_end_matches('/').to_owned();

        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()?;

        debug!(
            %base_url,
            "created Bright Sky client"
        );

        Ok(Self { http, base_url })
    }

    /// Fetches hourly weather data.
    ///
    /// The returned dataset retains the raw response internally so multiple
    /// typed series can be extracted without additional HTTP requests.
    #[instrument(
        name = "brightsky.weather",
        skip(self),
        fields(
            location = %query.location,
            start = %query.start,
            end = tracing::field::Empty,
            timezone = query.timezone.as_deref().unwrap_or("UTC"),
            units = %query.units,
            status = tracing::field::Empty,
            records = tracing::field::Empty,
            sources = tracing::field::Empty
        )
    )]
    pub async fn weather(&self, query: &WeatherQuery) -> Result<WeatherData, Error> {
        query.validate()?;

        let span = tracing::Span::current();

        if let Some(end) = query.end.as_ref() {
            span.record("end", tracing::field::display(end));
        }

        let endpoint = format!("{}/weather", self.base_url);
        let parameters = query.to_parameters();

        debug!(
            %endpoint,
            parameter_count = parameters.len(),
            "sending Bright Sky request"
        );

        let response = self.http.get(&endpoint).query(&parameters).send().await?;

        let status = response.status();

        span.record("status", status.as_u16() as u64);

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();

            warn!(
                %status,
                response_body = %body,
                "Bright Sky request failed"
            );

            return Err(Error::Api { status, body });
        }

        let response: WeatherResponse = response.json().await?;

        span.record("records", response.weather.len() as u64);

        span.record("sources", response.sources.len() as u64);

        debug!(
            records = response.weather.len(),
            sources = response.sources.len(),
            "received Bright Sky weather data"
        );

        let sources = response
            .sources
            .into_iter()
            .map(|source| (source.id(), source))
            .collect();

        Ok(WeatherData {
            units: query.units,
            records: response.weather,
            sources,
        })
    }

    /// Fetches one typed series.
    ///
    /// Use `weather()` directly when several metrics should be extracted from
    /// one HTTP response.
    #[instrument(
        name = "brightsky.series",
        skip(self, query),
        fields(metric = M::NAME)
    )]
    pub async fn series<M>(&self, query: &WeatherQuery) -> Result<TimeSeries<M>, Error>
    where
        M: Metric,
    {
        let weather = self.weather(query).await?;
        let series = weather.series::<M>();

        debug!(
            metric = M::NAME,
            samples = series.len(),
            unit = %series.unit(),
            "created typed weather series"
        );

        Ok(series)
    }
}

impl Default for BrightSky {
    fn default() -> Self {
        Self::new().expect("failed to construct the default Bright Sky client")
    }
}

/// A type-safe weather request.
#[derive(Clone, Debug)]
pub struct WeatherQuery {
    location: Location,
    start: DateTime<FixedOffset>,
    end: Option<DateTime<FixedOffset>>,
    timezone: Option<String>,
    units: UnitSystem,
    max_distance_m: Option<u32>,
}

impl WeatherQuery {
    /// Creates a request beginning at `start`.
    pub fn new(location: Location, start: DateTime<FixedOffset>) -> Self {
        Self {
            location,
            start,
            end: None,
            timezone: None,
            units: UnitSystem::Dwd,
            max_distance_m: None,
        }
    }

    /// Sets the end of the requested interval.
    pub fn until(mut self, end: DateTime<FixedOffset>) -> Self {
        self.end = Some(end);
        self
    }

    /// Sets the IANA timezone used for returned timestamps.
    pub fn timezone(mut self, timezone: impl Into<String>) -> Self {
        self.timezone = Some(timezone.into());
        self
    }

    /// Selects the physical unit system.
    pub fn units(mut self, units: UnitSystem) -> Self {
        self.units = units;
        self
    }

    /// Limits how far Bright Sky may search for a weather source.
    ///
    /// This option is only valid for coordinate locations.
    pub fn max_distance(mut self, meters: u32) -> Self {
        self.max_distance_m = Some(meters);
        self
    }

    pub fn location(&self) -> &Location {
        &self.location
    }

    pub fn start(&self) -> &DateTime<FixedOffset> {
        &self.start
    }

    pub fn end(&self) -> Option<&DateTime<FixedOffset>> {
        self.end.as_ref()
    }

    pub fn unit_system(&self) -> UnitSystem {
        self.units
    }

    #[instrument(
        name = "brightsky.validate_query",
        skip(self),
        fields(location = %self.location),
        level = "debug"
    )]
    fn validate(&self) -> Result<(), Error> {
        self.location.validate()?;

        if let Some(end) = self.end.as_ref()
            && end < &self.start
        {
            warn!(
                start = %self.start,
                %end,
                "weather query has an invalid time range"
            );

            return Err(Error::InvalidQuery(
                "end must not be earlier than start".into(),
            ));
        }

        if let Some(distance) = self.max_distance_m {
            if !matches!(self.location, Location::Coordinates { .. }) {
                return Err(Error::InvalidQuery(
                    "max_distance is only valid for coordinate locations".into(),
                ));
            }

            if distance > 500_000 {
                return Err(Error::InvalidQuery(
                    "max_distance must not exceed 500,000 metres".into(),
                ));
            }
        }

        if self
            .timezone
            .as_ref()
            .is_some_and(|timezone| timezone.trim().is_empty())
        {
            return Err(Error::InvalidQuery("timezone must not be empty".into()));
        }

        Ok(())
    }

    fn to_parameters(&self) -> Vec<(String, String)> {
        let mut parameters = vec![
            ("date".into(), self.start.to_rfc3339()),
            ("units".into(), self.units.as_api_value().into()),
        ];

        if let Some(end) = self.end.as_ref() {
            parameters.push(("last_date".into(), end.to_rfc3339()));
        }

        if let Some(timezone) = self.timezone.as_ref() {
            parameters.push(("tz".into(), timezone.clone()));
        }

        if let Some(distance) = self.max_distance_m {
            parameters.push(("max_dist".into(), distance.to_string()));
        }

        self.location.append_parameters(&mut parameters);

        parameters
    }
}

/// A location accepted by Bright Sky.
#[derive(Clone, Debug, PartialEq)]
pub enum Location {
    Coordinates { latitude: f64, longitude: f64 },
    DwdStation(String),
    WmoStation(String),
}

impl Location {
    pub fn coordinates(latitude: f64, longitude: f64) -> Self {
        Self::Coordinates {
            latitude,
            longitude,
        }
    }

    pub fn dwd_station(id: impl Into<String>) -> Self {
        Self::DwdStation(id.into())
    }

    pub fn wmo_station(id: impl Into<String>) -> Self {
        Self::WmoStation(id.into())
    }

    fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Coordinates {
                latitude,
                longitude,
            } => {
                if !latitude.is_finite() {
                    return Err(Error::InvalidQuery("latitude must be finite".into()));
                }

                if !longitude.is_finite() {
                    return Err(Error::InvalidQuery("longitude must be finite".into()));
                }

                if !(-90.0..=90.0).contains(latitude) {
                    return Err(Error::InvalidQuery(
                        "latitude must be between -90 and 90".into(),
                    ));
                }

                if !(-180.0..=180.0).contains(longitude) {
                    return Err(Error::InvalidQuery(
                        "longitude must be between -180 and 180".into(),
                    ));
                }
            }

            Self::DwdStation(id) | Self::WmoStation(id) if id.trim().is_empty() => {
                return Err(Error::InvalidQuery("station ID must not be empty".into()));
            }

            _ => {}
        }

        Ok(())
    }

    fn append_parameters(&self, parameters: &mut Vec<(String, String)>) {
        match self {
            Self::Coordinates {
                latitude,
                longitude,
            } => {
                parameters.push(("lat".into(), latitude.to_string()));

                parameters.push(("lon".into(), longitude.to_string()));
            }

            Self::DwdStation(id) => {
                parameters.push(("dwd_station_id".into(), id.clone()));
            }

            Self::WmoStation(id) => {
                parameters.push(("wmo_station_id".into(), id.clone()));
            }
        }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coordinates {
                latitude,
                longitude,
            } => {
                write!(formatter, "{latitude:.5},{longitude:.5}")
            }

            Self::DwdStation(id) => {
                write!(formatter, "dwd:{id}")
            }

            Self::WmoStation(id) => {
                write!(formatter, "wmo:{id}")
            }
        }
    }
}

/// Unit system requested from Bright Sky.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitSystem {
    #[default]
    Dwd,
    Si,
}

impl UnitSystem {
    fn as_api_value(self) -> &'static str {
        match self {
            Self::Dwd => "dwd",
            Self::Si => "si",
        }
    }
}

impl fmt::Display for UnitSystem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_api_value())
    }
}

/// A physical unit attached to a complete series.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Celsius,
    Kelvin,
    Hectopascal,
    Pascal,
    Percent,
    Millimetre,
    Metre,
    KilometresPerHour,
    MetresPerSecond,
    Degree,
    Minute,
    Second,
    KilowattHourPerSquareMetre,
    JoulePerSquareMetre,
    Dimensionless,
}

impl Unit {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Celsius => "°C",
            Self::Kelvin => "K",
            Self::Hectopascal => "hPa",
            Self::Pascal => "Pa",
            Self::Percent => "%",
            Self::Millimetre => "mm",
            Self::Metre => "m",
            Self::KilometresPerHour => "km/h",
            Self::MetresPerSecond => "m/s",
            Self::Degree => "°",
            Self::Minute => "min",
            Self::Second => "s",
            Self::KilowattHourPerSquareMetre => "kWh/m²",
            Self::JoulePerSquareMetre => "J/m²",
            Self::Dimensionless => "",
        }
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.symbol())
    }
}

/// Identifies a Bright Sky source.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SourceId(i64);

impl SourceId {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A timestamped homogeneous value with explicit provenance.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Sample<T> {
    pub timestamp: DateTime<FixedOffset>,
    pub value: T,

    /// Source that supplied this particular metric value.
    pub source_id: SourceId,
}

/// A typed metric.
///
/// The associated value type connects the metric type to its sample value.
/// The source field identifies the corresponding Bright Sky fallback-source
/// key.
pub trait Metric:
    private::Sealed + Sized + Send + Sync + 'static + std::fmt::Debug + Clone
{
    type Value: std::fmt::Debug + Clone;

    const NAME: &'static str;
    const SOURCE_FIELD: &'static str;

    fn unit(system: UnitSystem) -> Unit;

    #[doc(hidden)]
    fn extract(record: &RawWeatherRecord) -> Option<Self::Value>;
}

/// A homogeneous typed time series.
#[derive(Clone, Debug)]
pub struct TimeSeries<M>
where
    M: Metric,
{
    unit: Unit,
    samples: Vec<Sample<M::Value>>,
    marker: PhantomData<fn() -> M>,
}

impl<M> TimeSeries<M>
where
    M: Metric,
{
    fn new(unit_system: UnitSystem, samples: Vec<Sample<M::Value>>) -> Self {
        Self {
            unit: M::unit(unit_system),
            samples,
            marker: PhantomData,
        }
    }

    pub fn name(&self) -> &'static str {
        M::NAME
    }

    pub fn unit(&self) -> Unit {
        self.unit
    }

    pub fn samples(&self) -> &[Sample<M::Value>] {
        &self.samples
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Sample<M::Value>> {
        self.samples.iter()
    }

    pub fn first(&self) -> Option<&Sample<M::Value>> {
        self.samples.first()
    }

    pub fn last(&self) -> Option<&Sample<M::Value>> {
        self.samples.last()
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn into_samples(self) -> Vec<Sample<M::Value>> {
        self.samples
    }

    /// Transforms values while preserving timestamps and provenance.
    pub fn map_values<U>(&self, mut map: impl FnMut(&M::Value) -> U) -> Vec<Sample<U>> {
        self.samples
            .iter()
            .map(|sample| Sample {
                timestamp: sample.timestamp,
                value: map(&sample.value),
                source_id: sample.source_id,
            })
            .collect()
    }
}

impl<M> AsRef<[Sample<M::Value>]> for TimeSeries<M>
where
    M: Metric,
{
    fn as_ref(&self) -> &[Sample<M::Value>] {
        &self.samples
    }
}

impl<M> IntoIterator for TimeSeries<M>
where
    M: Metric,
{
    type Item = Sample<M::Value>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.samples.into_iter()
    }
}

impl<'a, M> IntoIterator for &'a TimeSeries<M>
where
    M: Metric,
{
    type Item = &'a Sample<M::Value>;
    type IntoIter = std::slice::Iter<'a, Sample<M::Value>>;

    fn into_iter(self) -> Self::IntoIter {
        self.samples.iter()
    }
}

/// A fetched Bright Sky response.
///
/// Raw heterogeneous records remain internal. Public typed series expose
/// homogeneous samples with explicit source IDs.
#[derive(Clone, Debug)]
pub struct WeatherData {
    units: UnitSystem,
    records: Vec<RawWeatherRecord>,
    sources: HashMap<SourceId, Source>,
}

impl WeatherData {
    pub fn unit_system(&self) -> UnitSystem {
        self.units
    }

    pub fn sources(&self) -> impl ExactSizeIterator<Item = &Source> {
        self.sources.values()
    }

    pub fn source(&self, source_id: SourceId) -> Option<&Source> {
        self.sources.get(&source_id)
    }

    pub fn source_for<T>(&self, sample: &Sample<T>) -> Option<&Source> {
        self.source(sample.source_id)
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Extracts a homogeneous typed series.
    ///
    /// Records where the metric is unavailable are omitted.
    pub fn series<M>(&self) -> TimeSeries<M>
    where
        M: Metric,
    {
        let samples = self
            .records
            .iter()
            .filter_map(|record| {
                let value = M::extract(record)?;

                Some(Sample {
                    timestamp: record.timestamp,
                    value,
                    source_id: record.effective_source::<M>(),
                })
            })
            .collect();

        TimeSeries::new(self.units, samples)
    }

    /// Extracts a typed series while preserving missing values.
    ///
    /// Missing samples use the primary record source because there is no
    /// actual metric value whose fallback source can be attributed.
    pub fn series_with_gaps<M>(&self) -> Vec<Sample<Option<M::Value>>>
    where
        M: Metric,
    {
        self.records
            .iter()
            .map(|record| {
                let value = M::extract(record);

                let source_id = if value.is_some() {
                    record.effective_source::<M>()
                } else {
                    record.source_id
                };

                Sample {
                    timestamp: record.timestamp,
                    value,
                    source_id,
                }
            })
            .collect()
    }
}

macro_rules! numeric_metric {
    (
        $(#[$meta:meta])*
        $type_name:ident,
        name = $name:literal,
        source = $source:literal,
        field = $field:ident,
        unit = $unit:expr
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug)]
        pub struct $type_name;

        impl private::Sealed for $type_name {}

        impl Metric for $type_name {
            type Value = f64;

            const NAME: &'static str = $name;
            const SOURCE_FIELD: &'static str = $source;

            fn unit(system: UnitSystem) -> Unit {
                ($unit)(system)
            }

            fn extract(
                record: &RawWeatherRecord,
            ) -> Option<Self::Value> {
                record.$field.map(IntoF64::into_f64)
            }
        }
    };
}

trait IntoF64 {
    fn into_f64(self) -> f64;
}

impl IntoF64 for f64 {
    fn into_f64(self) -> f64 {
        self
    }
}

impl IntoF64 for u8 {
    fn into_f64(self) -> f64 {
        f64::from(self)
    }
}

impl IntoF64 for u16 {
    fn into_f64(self) -> f64 {
        f64::from(self)
    }
}

impl IntoF64 for u32 {
    fn into_f64(self) -> f64 {
        f64::from(self)
    }
}

numeric_metric!(
    /// Temperature two metres above ground.
    Temperature,
    name = "temperature",
    source = "temperature",
    field = temperature,
    unit = |system| match system {
        UnitSystem::Dwd => Unit::Celsius,
        UnitSystem::Si => Unit::Kelvin,
    }
);

numeric_metric!(
    /// Dew point two metres above ground.
    DewPoint,
    name = "dew_point",
    source = "dew_point",
    field = dew_point,
    unit = |system| match system {
        UnitSystem::Dwd => Unit::Celsius,
        UnitSystem::Si => Unit::Kelvin,
    }
);

numeric_metric!(
    /// Relative humidity.
    RelativeHumidity,
    name = "relative_humidity",
    source = "relative_humidity",
    field = relative_humidity,
    unit = |_| Unit::Percent
);

numeric_metric!(
    /// Atmospheric pressure reduced to mean sea level.
    PressureMsl,
    name = "pressure_msl",
    source = "pressure_msl",
    field = pressure_msl,
    unit = |system| match system {
        UnitSystem::Dwd => Unit::Hectopascal,
        UnitSystem::Si => Unit::Pascal,
    }
);

numeric_metric!(
    /// Total cloud cover.
    CloudCover,
    name = "cloud_cover",
    source = "cloud_cover",
    field = cloud_cover,
    unit = |_| Unit::Percent
);

numeric_metric!(
    /// Visibility distance.
    Visibility,
    name = "visibility",
    source = "visibility",
    field = visibility,
    unit = |_| Unit::Metre
);

numeric_metric!(
    /// Precipitation during the previous hour.
    Precipitation,
    name = "precipitation",
    source = "precipitation",
    field = precipitation,
    unit = |_| Unit::Millimetre
);

numeric_metric!(
    /// Forecast precipitation probability.
    PrecipitationProbability,
    name = "precipitation_probability",
    source = "precipitation_probability",
    field = precipitation_probability,
    unit = |_| Unit::Percent
);

numeric_metric!(
    /// Solar irradiation during the previous hour.
    SolarIrradiation,
    name = "solar_irradiation",
    source = "solar",
    field = solar,
    unit = |system| match system {
        UnitSystem::Dwd => {
            Unit::KilowattHourPerSquareMetre
        }
        UnitSystem::Si => {
            Unit::JoulePerSquareMetre
        }
    }
);

numeric_metric!(
    /// Sunshine duration during the previous hour.
    SunshineDuration,
    name = "sunshine_duration",
    source = "sunshine",
    field = sunshine,
    unit = |system| match system {
        UnitSystem::Dwd => Unit::Minute,
        UnitSystem::Si => Unit::Second,
    }
);

numeric_metric!(
    /// Mean wind speed during the previous hour.
    WindSpeed,
    name = "wind_speed",
    source = "wind_speed",
    field = wind_speed,
    unit = |system| match system {
        UnitSystem::Dwd => {
            Unit::KilometresPerHour
        }
        UnitSystem::Si => {
            Unit::MetresPerSecond
        }
    }
);

numeric_metric!(
    /// Mean wind direction during the previous hour.
    WindDirection,
    name = "wind_direction",
    source = "wind_direction",
    field = wind_direction,
    unit = |_| Unit::Degree
);

numeric_metric!(
    /// Maximum wind-gust speed during the previous hour.
    WindGustSpeed,
    name = "wind_gust_speed",
    source = "wind_gust_speed",
    field = wind_gust_speed,
    unit = |system| match system {
        UnitSystem::Dwd => {
            Unit::KilometresPerHour
        }
        UnitSystem::Si => {
            Unit::MetresPerSecond
        }
    }
);

numeric_metric!(
    /// Direction of the maximum wind gust.
    WindGustDirection,
    name = "wind_gust_direction",
    source = "wind_gust_direction",
    field = wind_gust_direction,
    unit = |_| Unit::Degree
);

/// Categorical weather condition calculated by Bright Sky.
#[derive(Clone, Copy, Debug)]
pub struct Condition;

impl private::Sealed for Condition {}

impl Metric for Condition {
    type Value = WeatherCondition;

    const NAME: &'static str = "condition";
    const SOURCE_FIELD: &'static str = "condition";

    fn unit(_: UnitSystem) -> Unit {
        Unit::Dimensionless
    }

    fn extract(record: &RawWeatherRecord) -> Option<Self::Value> {
        record.condition
    }
}

/// A categorical weather condition.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WeatherCondition {
    Dry,
    Fog,
    Rain,
    Sleet,
    Snow,
    Hail,
    Thunderstorm,
}

impl fmt::Display for WeatherCondition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Dry => "dry",
            Self::Fog => "fog",
            Self::Rain => "rain",
            Self::Sleet => "sleet",
            Self::Snow => "snow",
            Self::Hail => "hail",
            Self::Thunderstorm => "thunderstorm",
        };

        formatter.write_str(value)
    }
}

/// A Bright Sky source, usually a station or forecast source.
#[derive(Clone, Debug, Deserialize)]
pub struct Source {
    id: SourceId,
    dwd_station_id: Option<String>,
    wmo_station_id: Option<String>,
    station_name: Option<String>,
    observation_type: ObservationType,
    first_record: DateTime<FixedOffset>,
    last_record: DateTime<FixedOffset>,
    lat: f64,
    lon: f64,
    height: f64,

    #[serde(default)]
    distance: Option<u32>,
}

impl Source {
    pub fn id(&self) -> SourceId {
        self.id
    }

    pub fn dwd_station_id(&self) -> Option<&str> {
        self.dwd_station_id.as_deref()
    }

    pub fn wmo_station_id(&self) -> Option<&str> {
        self.wmo_station_id.as_deref()
    }

    pub fn name(&self) -> Option<&str> {
        self.station_name.as_deref()
    }

    pub fn observation_type(&self) -> ObservationType {
        self.observation_type
    }

    pub fn first_record(&self) -> &DateTime<FixedOffset> {
        &self.first_record
    }

    pub fn last_record(&self) -> &DateTime<FixedOffset> {
        &self.last_record
    }

    pub fn latitude(&self) -> f64 {
        self.lat
    }

    pub fn longitude(&self) -> f64 {
        self.lon
    }

    pub fn height_m(&self) -> f64 {
        self.height
    }

    pub fn distance_m(&self) -> Option<u32> {
        self.distance
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ObservationType {
    Historical,
    Current,
    Synop,
    Forecast,
}

/// Internal representation of one Bright Sky hourly record.
#[doc(hidden)]
#[derive(Clone, Debug, Deserialize)]
pub struct RawWeatherRecord {
    timestamp: DateTime<FixedOffset>,
    source_id: SourceId,

    #[serde(default)]
    fallback_source_ids: HashMap<String, SourceId>,

    cloud_cover: Option<f64>,
    condition: Option<WeatherCondition>,
    dew_point: Option<f64>,
    pressure_msl: Option<f64>,
    relative_humidity: Option<u8>,
    temperature: Option<f64>,
    visibility: Option<u32>,

    precipitation: Option<f64>,
    precipitation_probability: Option<u8>,

    solar: Option<f64>,
    sunshine: Option<u32>,

    wind_direction: Option<u16>,
    wind_speed: Option<f64>,
    wind_gust_direction: Option<u16>,
    wind_gust_speed: Option<f64>,
}

impl RawWeatherRecord {
    fn effective_source<M>(&self) -> SourceId
    where
        M: Metric,
    {
        self.fallback_source_ids
            .get(M::SOURCE_FIELD)
            .copied()
            .unwrap_or(self.source_id)
    }
}

#[derive(Debug, Deserialize)]
struct WeatherResponse {
    weather: Vec<RawWeatherRecord>,
    sources: Vec<Source>,
}

mod private {
    pub trait Sealed {}
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid weather query: {0}")]
    InvalidQuery(String),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Bright Sky returned HTTP {status}: {body}")]
    Api { status: StatusCode, body: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timestamp(value: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(value).unwrap()
    }

    fn raw_record(time: &str, source_id: i64) -> RawWeatherRecord {
        RawWeatherRecord {
            timestamp: timestamp(time),
            source_id: SourceId::new(source_id),
            fallback_source_ids: HashMap::new(),

            cloud_cover: None,
            condition: None,
            dew_point: None,
            pressure_msl: None,
            relative_humidity: None,
            temperature: None,
            visibility: None,

            precipitation: None,
            precipitation_probability: None,

            solar: None,
            sunshine: None,

            wind_direction: None,
            wind_speed: None,
            wind_gust_direction: None,
            wind_gust_speed: None,
        }
    }

    #[test]
    fn extracts_typed_temperature_series() {
        let mut first = raw_record("2026-08-01T10:00:00+02:00", 1);

        first.temperature = Some(21.5);

        let second = raw_record("2026-08-01T11:00:00+02:00", 1);

        let weather = WeatherData {
            units: UnitSystem::Dwd,
            sources: HashMap::new(),
            records: vec![first, second],
        };

        let series = weather.series::<Temperature>();

        assert_eq!(series.name(), "temperature",);

        assert_eq!(series.unit(), Unit::Celsius,);

        assert_eq!(series.len(), 1);
        assert_eq!(series.samples()[0].value, 21.5);

        assert_eq!(series.samples()[0].source_id, SourceId::new(1),);
    }

    #[test]
    fn uses_metric_fallback_source() {
        let mut record = raw_record("2026-08-01T10:00:00+02:00", 1);

        record.temperature = Some(21.5);

        record
            .fallback_source_ids
            .insert("temperature".to_owned(), SourceId::new(2));

        let weather = WeatherData {
            units: UnitSystem::Dwd,
            sources: HashMap::new(),
            records: vec![record],
        };

        let series = weather.series::<Temperature>();

        assert_eq!(series.len(), 1);

        assert_eq!(series.samples()[0].source_id, SourceId::new(2),);
    }

    #[test]
    fn gap_preserving_series_keeps_missing_values() {
        let weather = WeatherData {
            units: UnitSystem::Dwd,
            sources: HashMap::new(),
            records: vec![raw_record("2026-08-01T10:00:00+02:00", 1)],
        };

        let samples = weather.series_with_gaps::<Temperature>();

        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].value, None);

        assert_eq!(samples[0].source_id, SourceId::new(1),);
    }

    #[test]
    fn map_values_preserves_source() {
        let series = TimeSeries::<Temperature>::new(
            UnitSystem::Dwd,
            vec![Sample {
                timestamp: timestamp("2026-08-01T10:00:00+02:00"),
                value: 21.5,
                source_id: SourceId::new(7),
            }],
        );

        let mapped = series.map_values(|value| value.round() as i32);

        assert_eq!(mapped[0].value, 22);

        assert_eq!(mapped[0].source_id, SourceId::new(7),);
    }

    #[test]
    fn rejects_invalid_coordinates() {
        let query = WeatherQuery::new(
            Location::coordinates(95.0, 11.0),
            timestamp("2026-08-01T00:00:00+02:00"),
        );

        assert!(matches!(query.validate(), Err(Error::InvalidQuery(_))));
    }

    #[test]
    fn rejects_reversed_time_range() {
        let query = WeatherQuery::new(
            Location::coordinates(49.4521, 11.0767),
            timestamp("2026-08-02T00:00:00+02:00"),
        )
        .until(timestamp("2026-08-01T00:00:00+02:00"));

        assert!(matches!(query.validate(), Err(Error::InvalidQuery(_))));
    }
}
