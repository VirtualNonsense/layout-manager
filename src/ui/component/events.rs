//! Built-in component event types.
//!
//! All three are generated with [`new_event!`] and are available for use by
//! any component via `Event::downcast_ref`.

use crate::data_source::end_points::brightsky::Temperature;
use crate::data_source::services::weather::WeatherUpdate;
use crate::new_event;
use crate::ui::command::{Direction2D, PointerEvent};
use std::time::Duration;

new_event!(MoveEvent, Direction2D);

new_event!(Submit);

new_event!(MouseEvent, PointerEvent);

new_event!(Tick, Duration);

new_event!(TemperatureUpdate, WeatherUpdate<Temperature>);
