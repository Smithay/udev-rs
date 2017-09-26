//!
//! libudev Bindings for Rust
//!

#![warn(missing_docs)]

extern crate libudev_sys as ffi;
extern crate libc;

pub use context::Context;
pub use device::{Device, DeviceType, Properties, Property, Attributes, Attribute};
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
pub trait FromRawWithContext<T: 'static> {
    /// Create an object from a given raw pointer and the matching context.
    ///
    /// The reference count will not be increased, be sure not to free this pointer.
    ///
    /// ## Unsafety
    ///
    /// The pointer has to be a valid reference to the expected underlying udev-struct or undefined
    /// behaviour might occur.
    ///
    /// If the context does not match the context that was used to create the given pointer
    /// undefined behaviour including use-after-free segfaults might occur.
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
