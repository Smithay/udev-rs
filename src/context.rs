use std::path::Path;

use ::{Device, FromRaw, FromRawWithContext};

/// A libudev context.
pub struct Context {
    udev: *mut ::ffi::udev
}

impl Clone for Context {
    fn clone(&self) -> Context {
        Context {
            udev: unsafe { ::ffi::udev_ref(self.udev) }
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            ::ffi::udev_unref(self.udev);
        }
    }
}

as_ffi!(Context, udev, ::ffi::udev);

impl FromRaw<::ffi::udev> for Context {
    unsafe fn from_raw(ptr: *mut ::ffi::udev) -> Context {
        Context {
            udev: ptr,
        }
    }
}

impl Context {
    /// Creates a new context.
    pub fn new() -> ::Result<Self> {
        let ptr = try_alloc!(unsafe { ::ffi::udev_new() });
        Ok(unsafe { Context::from_raw(ptr) })
    }

    /// Creates a device for a given syspath.
    ///
    /// The `syspath` parameter should be a path to the device file within the `sysfs` file system,
    /// e.g., `/sys/devices/virtual/tty/tty0`.
    pub fn device_from_syspath(&self, syspath: &Path) -> ::Result<Device> {
        let syspath = try!(::util::os_str_to_cstring(syspath));

        let ptr = try_alloc!(unsafe {
            ::ffi::udev_device_new_from_syspath(self.udev, syspath.as_ptr())
        });

        Ok(unsafe { Device::from_raw(self, ptr) })
    }
}
