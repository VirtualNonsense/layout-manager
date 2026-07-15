use std::any::Any;

use crate::ui::{Direction2D, command::PointerEvent};

pub trait Event: Any + Send + Sync + std::fmt::Debug {
    fn event_name(&self) -> &'static str;
    fn box_clone(&self) -> Box<dyn Event>;
    fn as_any(&self) -> &dyn Any;
}

impl dyn Event {
    /// Tries to cast a dyn event to a concret type.
    pub fn downcast_ref<T: Event + 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}

impl Clone for Box<dyn Event> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

macro_rules! new_event {
    // -----------------------------
    // Unit struct: new_event!(Foo);
    // -----------------------------
    ($name:ident) => {
        #[derive(Debug, Clone)]
        pub struct $name;

        impl Event for $name {
            fn event_name(&self) -> &'static str {
                stringify!($name)
            }

            fn box_clone(&self) -> Box<dyn Event>{
                Box::new(self.clone())
            }

            fn as_any(&self) -> &dyn Any{
                self
            }
        }
    };

    // --------------------------------------------------
    // Tuple struct: new_event!(Foo, i32, String, bool);
    // --------------------------------------------------
    ($name:ident, $($parameter:ty),+ $(,)?) => {
        #[derive(Debug, Clone)]
        pub struct $name($(pub $parameter),+);

        impl Event for $name {
            fn event_name(&self) -> &'static str {
                stringify!($name)
            }
            fn box_clone(&self) -> Box<dyn Event>{
                Box::new(self.clone())
            }
            fn as_any(&self) -> &dyn Any{
                self
            }
        }
    };

    // -------------------------------------------------------------
    // Named-field struct: new_event!(Foo { x: i32, y: String });
    // -------------------------------------------------------------
    ($name:ident { $($field:ident : $ty:ty),+ $(,)? }) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            $(pub $field: $ty),+
        }

        impl Event for $name {
            fn event_name(&self) -> &'static str {
                stringify!($name)
            }
            fn box_clone(&self) -> Box<dyn Event>{
                Box::new(self.clone())
            }
            fn as_any(&self) -> &dyn Any{
                self
            }
        }
    };

    // --------------------------------------------------------------
    // Enum: new_event!(enum Foo { A, B(i32, String), C { x: i32 } });
    // --------------------------------------------------------------
    (enum $name:ident { $($variant:ident $( ( $($t_param:ty),+ ) )? $( { $($v_field:ident : $v_ty:ty),+ } )? ),+ $(,)? }) => {
        #[derive(Debug, Clone)]
        pub enum $name {
            $(
                $variant $( ( $($t_param),+ ) )? $( { $($v_field : $v_ty),+ } )?
            ),+
        }

        impl Event for $name {
            fn event_name(&self) -> &'static str {
                match self {
                    $(
                        $name::$variant { .. } => concat!(stringify!($name),"::",stringify!($variant)),
                    )+
                }
            }
            fn box_clone(&self) -> Box<dyn Event>{
                Box::new(self.clone())
            }
            fn as_any(&self) -> &dyn Any{
                self
            }
        }
    };
}

new_event!(MoveEvent, Direction2D);

new_event!(Submit);

new_event!(MouseEvent, PointerEvent);
