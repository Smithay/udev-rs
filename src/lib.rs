extern crate libudev_sys as ffi;
extern crate libc;

pub use context::{Context};
pub use device::{Device,Properties,Property,Attributes,Attribute};
pub use enumerator::{Enumerator,Devices};
pub use error::{Result,Error,ErrorKind};
pub use monitor::{MonitorSpec,Monitor,EventType,MonitorEvent};

macro_rules! try_alloc {
    ($exp:expr) => {{
        let ptr = $exp;

        if ptr.is_null() {
            return Err(::error::new(::error::ErrorKind::NoMem));
        }

        ptr
    }}
}

mod context;
mod device;
mod enumerator;
mod error;
mod monitor;

mod handle;
mod util;
