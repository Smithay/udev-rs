use std::path::Path;

use libc::{c_char, dev_t};

use ::{Device, DeviceType, FromRaw, FromRawWithContext};

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

    /// Creates a device for a given device type and number.
    pub fn device_from_devnum(&self, dev_type: DeviceType, dev_num: dev_t) -> ::Result<Device> {
        let ptr = try_alloc!(unsafe {
            ::ffi::udev_device_new_from_devnum(self.udev, dev_type as u8 as c_char, dev_num)
        });

        Ok(unsafe { Device::from_raw(self, ptr) })
    }

    /// Creates a device from a given subsystem and sysname.
    pub fn device_from_subsystem_sysname(&self, subsystem: &Path, syspath: &Path) -> ::Result<Device> {
        let subsystem = try!(::util::os_str_to_cstring(subsystem));
        let syspath = try!(::util::os_str_to_cstring(syspath));

        let ptr = try_alloc!(unsafe {
            ::ffi::udev_device_new_from_subsystem_sysname(self.udev, subsystem.as_ptr(), syspath.as_ptr())
        });

        Ok(unsafe { Device::from_raw(self, ptr) })
    }

    /// Creates a device from a given device id.
    ///
    /// The device id should be in one of these formats:
    ///
    /// - `b8:2` - block device major:minor
    /// - `c128:1` - char device major:minor
    /// - `n3` - network device ifindex
    /// - `+sound:card29` - kernel driver core subsystem:device name
    pub fn device_from_device_id(&self, device_id: &Path) -> ::Result<Device> {
        let device_id = try!(::util::os_str_to_cstring(device_id));

        let ptr = try_alloc!(unsafe {
            ::ffi::udev_device_new_from_device_id(self.udev, device_id.as_ptr())
        });

        Ok(unsafe { Device::from_raw(self, ptr) })
    }

    /// Creates a device from the current environment (see environ(7)).
    ///
    /// Each key-value pair is interpreted in the same way as if it was received in an uevent
    /// (see udev_monitor_receive_device(3)). The keys DEVPATH, SUBSYSTEM, ACTION, and SEQNUM are mandatory.
    pub fn device_from_environment(&self) -> ::Result<Device> {
        let ptr = try_alloc!(unsafe {
            ::ffi::udev_device_new_from_environment(self.udev)
        });

        Ok(unsafe { Device::from_raw(self, ptr) })
    }
}
