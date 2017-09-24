extern crate libudev_sys as ffi;
extern crate libc;

pub use context::Context;
pub use device::{Device, Properties, Property, Attributes, Attribute};
pub use enumerator::{Enumerator, Devices};
pub use error::{Result, Error, ErrorKind};
pub use monitor::{MonitorBuilder, MonitorSocket, EventType, Event};

macro_rules! try_alloc {
    ($exp:expr) => {{
        let ptr = $exp;

        if ptr.is_null() {
            return Err(::error::from_errno(::libc::ENOMEM));
        }

        ptr
    }}
}

pub trait AsRaw<T: 'static> {
    fn as_raw(&self) -> *mut T;
    fn into_raw(self) -> *mut T;
}

pub trait FromRaw<T: 'static> {
    unsafe fn from_raw(ptr: *mut T) -> Self;
}

pub trait FromRawWithContext<T: 'static> {
    unsafe fn from_raw(context: &Context, ptr: *mut T) -> Self;
}

macro_rules! as_ffi {
    ($struct_:ident, $field:ident, $type_:ty) => {
        impl $crate::AsRaw<$type_> for $struct_ {
            fn as_raw(&self) -> *mut $type_ {
                self.$field
            }

            fn into_raw(self) -> *mut $type_ {
                self.$field
            }
        }
    }
}

mod context;
mod device;
mod enumerator;
mod error;
mod monitor;
mod util;
