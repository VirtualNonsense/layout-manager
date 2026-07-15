use std::any::Any;

/// Trait implemented by every component-routable event.
///
/// Use the `new_event!` macro to derive this for concrete event types — it
/// automatically generates the boilerplate (`event_name`, `box_clone`, `as_any`).
pub trait Event: Any + Send + Sync + std::fmt::Debug {
    fn event_name(&self) -> &'static str;
    fn box_clone(&self) -> Box<dyn Event>;
    fn as_any(&self) -> &dyn Any;
}

impl dyn Event {
    /// Tries to downcast a `dyn Event` to a concrete type.
    pub fn downcast_ref<T: Event + 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}

impl Clone for Box<dyn Event> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

/// Derive [`Event`] for a concrete event type.
///
/// # Forms
///
/// ```rust,ignore
/// // Unit struct
/// new_event!(Submit);
///
/// // Tuple struct (one or more fields)
/// new_event!(MoveEvent, Direction2D);
///
/// // Named-field struct
/// new_event!(Resize { width: u16, height: u16 });
///
/// // Enum
/// new_event!(enum ScrollEvent { Up, Down, Left, Right });
/// ```
#[macro_export]
macro_rules! new_event {
    // Unit struct: new_event!(Foo);
    ($name:ident) => {
        #[derive(Debug, Clone)]
        pub struct $name;

        impl $crate::event::component::Event for $name {
            fn event_name(&self) -> &'static str {
                stringify!($name)
            }
            fn box_clone(&self) -> Box<dyn $crate::event::component::Event> {
                Box::new(self.clone())
            }
            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
        }
    };

    // Tuple struct: new_event!(Foo, i32, String);
    ($name:ident, $($parameter:ty),+ $(,)?) => {
        #[derive(Debug, Clone)]
        pub struct $name($(pub $parameter),+);

        impl $crate::event::component::Event for $name {
            fn event_name(&self) -> &'static str {
                stringify!($name)
            }
            fn box_clone(&self) -> Box<dyn $crate::event::component::Event> {
                Box::new(self.clone())
            }
            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
        }
    };

    // Named-field struct: new_event!(Foo { x: i32, y: String });
    ($name:ident { $($field:ident : $ty:ty),+ $(,)? }) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            $(pub $field: $ty),+
        }

        impl $crate::event::component::Event for $name {
            fn event_name(&self) -> &'static str {
                stringify!($name)
            }
            fn box_clone(&self) -> Box<dyn $crate::event::component::Event> {
                Box::new(self.clone())
            }
            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
        }
    };

    // Enum: new_event!(enum Foo { A, B(i32), C { x: i32 } });
    (enum $name:ident { $($variant:ident $( ( $($t_param:ty),+ ) )? $( { $($v_field:ident : $v_ty:ty),+ } )? ),+ $(,)? }) => {
        #[derive(Debug, Clone)]
        pub enum $name {
            $(
                $variant $( ( $($t_param),+ ) )? $( { $($v_field : $v_ty),+ } )?
            ),+
        }

        impl $crate::event::component::Event for $name {
            fn event_name(&self) -> &'static str {
                match self {
                    $(
                        $name::$variant { .. } => concat!(stringify!($name), "::", stringify!($variant)),
                    )+
                }
            }
            fn box_clone(&self) -> Box<dyn $crate::event::component::Event> {
                Box::new(self.clone())
            }
            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
        }
    };
}
