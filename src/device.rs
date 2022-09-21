use std::str;

use std::ffi::{CStr, OsStr};
use std::io::Result;
use std::marker::PhantomData;
use std::path::Path;
use std::ptr;
use std::str::FromStr;

use libc::{c_char, dev_t};

use list::{Entry, EntryList};
use Udev;
use {ffi, util};

use {AsRaw, FromRaw};

/// A structure that provides access to sysfs/kernel devices.
pub struct Device {
    udev: Udev,
    device: *mut ffi::udev_device,
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Device")
            .field("initialized", &self.is_initialized())
            .field("device_major_minor_number", &self.devnum())
            .field("system_path", &self.syspath())
            .field("device_path", &self.devpath())
            .field("device_node", &self.devnode())
            .field("subsystem_name", &self.subsystem())
            .field("system_name", &self.sysname())
            .field("instance_number", &self.sysnum())
            .field("device_type", &self.devtype())
            .field("driver", &self.driver())
            .field("action", &self.action())
            .field("parent", &self.parent())
            .finish()
    }
}

impl Clone for Device {
    fn clone(&self) -> Self {
        Self {
            udev: self.udev.clone(),
            device: unsafe { ffi::udev_device_ref(self.device) },
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            ffi::udev_device_unref(self.device);
        }
    }
}

as_ffi_with_context!(Device, device, ffi::udev_device, ffi::udev_device_ref);

/// A convenience alias for a list of properties, bound to a device.
pub type Properties<'a> = EntryList<'a, Device>;

/// A convenience alias for a list of attributes, bound to a device.
pub struct Attributes<'a> {
    entries: EntryList<'a, Device>,
    device: &'a Device,
}

impl Device {
    /// Creates a device for a given syspath.
    ///
    /// The `syspath` parameter should be a path to the device file within the `sysfs` file system,
    /// e.g., `/sys/devices/virtual/tty/tty0`.
    pub fn from_syspath(syspath: &Path) -> Result<Self> {
        // Create a new Udev context for this device
        // It would be more efficient to allow callers to create just one context and use multiple
        // devices, however that would be an API-breaking change.
        //
        // When devices are enumerated using an `Enumerator`, it will use
        // `from_syspath_with_context` which can reuse the existing `Udev` context to avoid this
        // extra overhead.
        let udev = Udev::new()?;

        Self::from_syspath_with_context(udev, syspath)
    }

    /// Creates a device for a given syspath, using an existing `Udev` instance rather than
    /// creating one automatically.
    ///
    /// The `syspath` parameter should be a path to the device file within the `sysfs` file system,
    /// e.g., `/sys/devices/virtual/tty/tty0`.
    pub fn from_syspath_with_context(udev: Udev, syspath: &Path) -> Result<Self> {
        let syspath = util::os_str_to_cstring(syspath)?;

        let ptr = try_alloc!(unsafe {
            ffi::udev_device_new_from_syspath(udev.as_raw(), syspath.as_ptr())
        });

        Ok(Self::from_raw(udev, ptr))
    }

    /// Creates a rust `Device` given an already created libudev `ffi::udev_device*` and a
    /// corresponding `Udev` instance from which the device was created.
    ///
    /// This guarantees that the `Udev` will live longer than the corresponding `Device`
    pub(crate) fn from_raw(udev: Udev, ptr: *mut ffi::udev_device) -> Self {
        Self { udev, device: ptr }
    }

    /// Checks whether the device has already been handled by udev.
    ///
    /// When a new device is connected to the system, udev initializes the device by setting
    /// permissions, renaming network devices, and possibly other initialization routines. This
    /// method returns `true` if udev has performed all of its work to initialize this device.
    ///
    /// This method only applies to devices with device nodes or network interfaces. All other
    /// devices return `true` by default.
    pub fn is_initialized(&self) -> bool {
        unsafe { ffi::udev_device_get_is_initialized(self.device) > 0 }
    }

    /// Gets the device's major/minor number.
    pub fn devnum(&self) -> Option<dev_t> {
        match unsafe { ffi::udev_device_get_devnum(self.device) } {
            0 => None,
            n => Some(n),
        }
    }

    /// Returns the syspath of the device.
    ///
    /// The path is an absolute path and includes the sys mount point. For example, the syspath for
    /// `tty0` could be `/sys/devices/virtual/tty/tty0`, which includes the sys mount point,
    /// `/sys`.
    pub fn syspath(&self) -> &Path {
        Path::new(unsafe {
            util::ptr_to_os_str_unchecked(ffi::udev_device_get_syspath(self.device))
        })
    }

    /// Returns the kernel devpath value of the device.
    ///
    /// The path does not contain the sys mount point, but does start with a `/`. For example, the
    /// devpath for `tty0` could be `/devices/virtual/tty/tty0`.
    pub fn devpath(&self) -> &OsStr {
        unsafe { util::ptr_to_os_str_unchecked(ffi::udev_device_get_devpath(self.device)) }
    }

    /// Returns the path to the device node belonging to the device.
    ///
    /// The path is an absolute path and starts with the device directory. For example, the device
    /// node for `tty0` could be `/dev/tty0`.
    pub fn devnode(&self) -> Option<&Path> {
        unsafe { util::ptr_to_os_str(ffi::udev_device_get_devnode(self.device)) }
            .map(|path| Path::new(path))
    }

    /// Returns the parent of the device.
    pub fn parent(&self) -> Option<Self> {
        let ptr = unsafe { ffi::udev_device_get_parent(self.device) };

        if ptr.is_null() {
            return None;
        }

        Some(Self::from_raw(self.udev.clone(), unsafe {
            ffi::udev_device_ref(ptr)
        }))
    }

    /// Returns the parent of the device with the matching subsystem and devtype if any.
    pub fn parent_with_subsystem<T: AsRef<OsStr>>(&self, subsystem: T) -> Result<Option<Self>> {
        let subsystem = util::os_str_to_cstring(subsystem)?;
        let ptr = unsafe {
            ffi::udev_device_get_parent_with_subsystem_devtype(
                self.device,
                subsystem.as_ptr(),
                ptr::null(),
            )
        };

        if ptr.is_null() {
            return Ok(None);
        }

        Ok(Some(Self::from_raw(self.udev.clone(), unsafe {
            ffi::udev_device_ref(ptr)
        })))
    }

    /// Returns the parent of the device with the matching subsystem and devtype if any.
    pub fn parent_with_subsystem_devtype<T: AsRef<OsStr>, U: AsRef<OsStr>>(
        &self,
        subsystem: T,
        devtype: U,
    ) -> Result<Option<Self>> {
        let subsystem = util::os_str_to_cstring(subsystem)?;
        let devtype = util::os_str_to_cstring(devtype)?;
        let ptr = unsafe {
            ffi::udev_device_get_parent_with_subsystem_devtype(
                self.device,
                subsystem.as_ptr(),
                devtype.as_ptr(),
            )
        };

        if ptr.is_null() {
            return Ok(None);
        }

        Ok(Some(Self::from_raw(self.udev.clone(), unsafe {
            ffi::udev_device_ref(ptr)
        })))
    }

    /// Returns the subsystem name of the device.
    ///
    /// The subsystem name is a string that indicates which kernel subsystem the device belongs to.
    /// Examples of subsystem names are `tty`, `vtconsole`, `block`, `scsi`, and `net`.
    pub fn subsystem(&self) -> Option<&OsStr> {
        unsafe { util::ptr_to_os_str(ffi::udev_device_get_subsystem(self.device)) }
    }

    /// Returns the kernel device name for the device.
    ///
    /// The sysname is a string that differentiates the device from others in the same subsystem.
    /// For example, `tty0` is the sysname for a TTY device that differentiates it from others,
    /// such as `tty1`.
    pub fn sysname(&self) -> &OsStr {
        unsafe { util::ptr_to_os_str_unchecked(ffi::udev_device_get_sysname(self.device)) }
    }

    /// Returns the instance number of the device.
    ///
    /// The instance number is used to differentiate many devices of the same type. For example,
    /// `/dev/tty0` and `/dev/tty1` are both TTY devices but have instance numbers of 0 and 1,
    /// respectively.
    ///
    /// Some devices don't have instance numbers, such as `/dev/console`, in which case the method
    /// returns `None`.
    pub fn sysnum(&self) -> Option<usize> {
        let ptr = unsafe { ffi::udev_device_get_sysnum(self.device) };

        if ptr.is_null() {
            return None;
        }

        match str::from_utf8(unsafe { CStr::from_ptr(ptr) }.to_bytes()) {
            Err(_) => None,
            Ok(s) => FromStr::from_str(s).ok(),
        }
    }

    /// Returns the devtype name of the device.
    pub fn devtype(&self) -> Option<&OsStr> {
        unsafe { util::ptr_to_os_str(ffi::udev_device_get_devtype(self.device)) }
    }

    /// Returns the name of the kernel driver attached to the device.
    pub fn driver(&self) -> Option<&OsStr> {
        unsafe { util::ptr_to_os_str(ffi::udev_device_get_driver(self.device)) }
    }

    /// Retreives the value of a device property.
    pub fn property_value<T: AsRef<OsStr>>(&self, property: T) -> Option<&OsStr> {
        let prop = match util::os_str_to_cstring(property) {
            Ok(s) => s,
            Err(_) => return None,
        };

        unsafe {
            util::ptr_to_os_str(ffi::udev_device_get_property_value(
                self.device,
                prop.as_ptr(),
            ))
        }
    }

    /// Retreives the value of a device attribute.
    pub fn attribute_value<T: AsRef<OsStr>>(&self, attribute: T) -> Option<&OsStr> {
        let attr = match util::os_str_to_cstring(attribute) {
            Ok(s) => s,
            Err(_) => return None,
        };

        unsafe {
            util::ptr_to_os_str(ffi::udev_device_get_sysattr_value(
                self.device,
                attr.as_ptr(),
            ))
        }
    }

    /// Sets the value of a device attribute.
    pub fn set_attribute_value<T: AsRef<OsStr>, U: AsRef<OsStr>>(
        &mut self,
        attribute: T,
        value: U,
    ) -> Result<()> {
        let attribute = util::os_str_to_cstring(attribute)?;
        let value = util::os_str_to_cstring(value)?;

        util::errno_to_result(unsafe {
            ffi::udev_device_set_sysattr_value(
                self.device,
                attribute.as_ptr(),
                value.as_ptr() as *mut c_char,
            )
        })
    }

    /// Returns an iterator over the device's properties.
    ///
    /// ## Example
    ///
    /// This example prints out all of a device's properties:
    ///
    /// ```no_run
    /// # use std::path::Path;
    /// # let device = udev::Device::from_syspath(Path::new("/sys/devices/virtual/tty/tty0")).unwrap();
    /// for property in device.properties() {
    ///     println!("{:?} = {:?}", property.name(), property.value());
    /// }
    /// ```
    pub fn properties(&self) -> Properties {
        Properties {
            entry: unsafe { ffi::udev_device_get_properties_list_entry(self.device) },
            phantom: PhantomData,
        }
    }

    /// Returns an iterator over the device's attributes.
    ///
    /// ## Example
    ///
    /// This example prints out all of a device's attributes:
    ///
    /// ```no_run
    /// # use std::path::Path;
    /// # let device = udev::Device::from_syspath(Path::new("/sys/devices/virtual/tty/tty0")).unwrap();
    /// for attribute in device.attributes() {
    ///     println!("{:?} = {:?}", attribute.name(), attribute.value());
    /// }
    /// ```
    pub fn attributes(&self) -> Attributes {
        Attributes {
            entries: EntryList {
                entry: unsafe { ffi::udev_device_get_sysattr_list_entry(self.device) },
                phantom: PhantomData,
            },
            device: self,
        }
    }

    /// Returns the device action for the device.
    pub fn action(&self) -> Option<&OsStr> {
        unsafe { util::ptr_to_os_str(ffi::udev_device_get_action(self.device)) }
    }
}

impl<'a> Iterator for Attributes<'a> {
    type Item = Entry<'a>;

    // The list of sysattr entries only contains the attribute names, with
    // the values being empty. To get the value, each has to be queried.
    fn next(&mut self) -> Option<Entry<'a>> {
        match self.entries.next() {
            Some(Entry { name, value: _ }) => Some(Entry {
                name,
                value: self.device.attribute_value(name),
            }),
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}
