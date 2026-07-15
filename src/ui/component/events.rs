//! Built-in component event types.
//!
//! All three are generated with [`new_event!`] and are available for use by
//! any component via `Event::downcast_ref`.

use crate::new_event;
use crate::ui::command::{Direction2D, PointerEvent};

new_event!(MoveEvent, Direction2D);

new_event!(Submit);

new_event!(MouseEvent, PointerEvent);
