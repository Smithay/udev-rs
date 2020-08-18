//!
//! libudev Bindings for Rust
//!

#![warn(missing_docs)]

extern crate libc;
pub extern crate libudev_sys as ffi;
#[cfg(feature = "mio")]
extern crate mio;

pub use device::{Attribute, Attributes, Device, Properties, Property};
pub use enumerator::{Devices, Enumerator};
pub use monitor::{Builder as MonitorBuilder, Event, EventType, Socket as MonitorSocket};
pub(crate) use udev::Udev;

macro_rules! try_alloc {
    ($exp:expr) => {{
        let ptr = $exp;

        if ptr.is_null() {
            return Err(std::io::Error::last_os_error());
        }

        ptr
    }};
}

/// Receive the underlying raw pointer
pub trait AsRaw<T: 'static> {
    /// Get a reference of the underlying struct.
    ///
    /// The reference count will not be increased.
    fn as_raw(&self) -> *mut T;
    /// Convert the object into the underlying pointer.
    ///
    /// You are responsible for freeing the object.
    fn into_raw(self) -> *mut T;
}

/// Convert from a raw pointer
pub trait FromRaw<T: 'static> {
    /// Create an object from a given raw pointer.
    ///
    /// The reference count will not be increased, be sure not to free this pointer.
    ///
    /// ## Unsafety
    ///
    /// The pointer has to be a valid reference to the expected underlying udev-struct or undefined
    /// behaviour might occur.
    unsafe fn from_raw(ptr: *mut T) -> Self;
}

/// Convert from a raw pointer and the matching context
macro_rules! as_ffi {
    ($struct_:ident, $field:ident, $type_:ty) => {
        as_raw!($struct_, $field, $type_);
        from_raw!($struct_, $field, $type_);
    };
}

macro_rules! as_raw {
    ($struct_:ident, $field:ident, $type_:ty) => {
        impl $crate::AsRaw<$type_> for $struct_ {
            fn as_raw(&self) -> *mut $type_ {
                self.$field
            }

            fn into_raw(self) -> *mut $type_ {
                self.$field
            }
        }
    };
}

macro_rules! from_raw {
    ($struct_:ident, $field:ident, $type_:ty) => {
        impl $crate::FromRaw<$type_> for $struct_ {
            unsafe fn from_raw(t: *mut $type_) -> Self {
                Self { $field: t }
            }
        }
    };
}

mod device;
mod enumerator;
mod monitor;
mod udev;
mod util;
