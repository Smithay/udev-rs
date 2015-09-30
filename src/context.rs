use std::path::Path;

use ::error::{Error};
use ::device::{Device};
use ::handle::{Handle};

pub struct Context {
    udev: *mut ::ffi::udev
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { ::ffi::udev_unref(self.udev) };
    }
}

impl Handle<::ffi::udev> for Context {
    fn as_ptr(&self) -> *mut ::ffi::udev {
        self.udev
    }
}

impl Context {
    pub fn new() -> Result<Self,Error> {
        let ptr = try_alloc!(unsafe { ::ffi::udev_new() });

        Ok(Context { udev: ptr })
    }

    pub fn device_from_syspath(&self, syspath: &Path) -> Result<Device,Error> {
        let syspath = try!(::util::os_str_to_cstring(syspath));

        let ptr = try_alloc!(unsafe {
            ::ffi::udev_device_new_from_syspath(self.udev, syspath.as_ptr())
        });

        Ok(::device::new(self, ptr))
    }
}
