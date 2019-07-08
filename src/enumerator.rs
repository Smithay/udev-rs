use std::ffi::OsStr;
use std::path::Path;
use std::ptr;

use {ffi, util};

use {AsRaw, Device, FromRaw, Result};

/// An enumeration context.
///
/// An Enumerator scans `/sys` for devices matching its filters. Filters are added to an Enumerator
/// by calling its `match_*` and `nomatch_*` methods. After the filters are setup, the
/// `scan_devices()` method finds devices in `/sys` that match the filters.
pub struct Enumerator {
    enumerator: *mut ffi::udev_enumerate,
}

impl Clone for Enumerator {
    fn clone(&self) -> Self {
        unsafe { Self::from_raw(ffi::udev_enumerate_ref(self.enumerator)) }
    }
}

impl Drop for Enumerator {
    fn drop(&mut self) {
        unsafe { ffi::udev_enumerate_unref(self.enumerator) };
    }
}

as_ffi!(Enumerator, enumerator, ffi::udev_enumerate);

impl Enumerator {
    /// Creates a new Enumerator.
    pub fn new() -> Result<Self> {
        let ptr = try_alloc!(unsafe { ffi::udev_enumerate_new(ptr::null_mut()) });
        Ok(unsafe { Self::from_raw(ptr) })
    }

    /// Adds a filter that matches only initialized devices.
    pub fn match_is_initialized(&mut self) -> Result<()> {
        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_match_is_initialized(self.enumerator)
        })
    }

    /// Adds a filter that matches only devices that belong to the given kernel subsystem.
    pub fn match_subsystem<T: AsRef<OsStr>>(&mut self, subsystem: T) -> Result<()> {
        let subsystem = util::os_str_to_cstring(subsystem)?;

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_match_subsystem(self.enumerator, subsystem.as_ptr())
        })
    }

    /// Adds a filter that matches only devices with the given attribute value.
    pub fn match_attribute<T: AsRef<OsStr>, U: AsRef<OsStr>>(
        &mut self,
        attribute: T,
        value: U,
    ) -> Result<()> {
        let attribute = util::os_str_to_cstring(attribute)?;
        let value = util::os_str_to_cstring(value)?;

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_match_sysattr(
                self.enumerator,
                attribute.as_ptr(),
                value.as_ptr(),
            )
        })
    }

    /// Adds a filter that matches only devices with the given kernel device name.
    pub fn match_sysname<T: AsRef<OsStr>>(&mut self, sysname: T) -> Result<()> {
        let sysname = util::os_str_to_cstring(sysname)?;

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_match_sysname(self.enumerator, sysname.as_ptr())
        })
    }

    /// Adds a filter that matches only devices with the given property value.
    pub fn match_property<T: AsRef<OsStr>, U: AsRef<OsStr>>(
        &mut self,
        property: T,
        value: U,
    ) -> Result<()> {
        let property = util::os_str_to_cstring(property)?;
        let value = util::os_str_to_cstring(value)?;

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_match_property(
                self.enumerator,
                property.as_ptr(),
                value.as_ptr(),
            )
        })
    }

    /// Adds a filter that matches only devices with the given tag.
    pub fn match_tag<T: AsRef<OsStr>>(&mut self, tag: T) -> Result<()> {
        let tag = util::os_str_to_cstring(tag)?;

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_match_tag(self.enumerator, tag.as_ptr())
        })
    }

    /// Includes the parent device and all devices in the subtree of the parent device.
    pub fn match_parent(&mut self, parent: &Device) -> Result<()> {
        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_match_parent(self.enumerator, parent.as_raw())
        })
    }

    /// Adds a filter that matches only devices that don't belong to the given kernel subsystem.
    pub fn nomatch_subsystem<T: AsRef<OsStr>>(&mut self, subsystem: T) -> Result<()> {
        let subsystem = util::os_str_to_cstring(subsystem)?;

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_nomatch_subsystem(self.enumerator, subsystem.as_ptr())
        })
    }

    /// Adds a filter that matches only devices that don't have the the given attribute value.
    pub fn nomatch_attribute<T: AsRef<OsStr>, U: AsRef<OsStr>>(
        &mut self,
        attribute: T,
        value: U,
    ) -> Result<()> {
        let attribute = util::os_str_to_cstring(attribute)?;
        let value = util::os_str_to_cstring(value)?;

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_nomatch_sysattr(
                self.enumerator,
                attribute.as_ptr(),
                value.as_ptr(),
            )
        })
    }

    /// Includes the device with the given syspath.
    pub fn add_syspath(&mut self, syspath: &Path) -> Result<()> {
        let syspath = util::os_str_to_cstring(syspath)?;

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_syspath(self.enumerator, syspath.as_ptr())
        })
    }

    /// Scans `/sys` for devices matching the attached filters.
    ///
    /// The devices will be sorted in dependency order.
    pub fn scan_devices(&mut self) -> Result<Devices> {
        util::errno_to_result(unsafe { ffi::udev_enumerate_scan_devices(self.enumerator) })?;

        Ok(Devices {
            entry: unsafe { ffi::udev_enumerate_get_list_entry(self.enumerator) },
        })
    }
}

/// Iterator over devices.
pub struct Devices {
    entry: *mut ffi::udev_list_entry,
}

impl Iterator for Devices {
    type Item = Device;

    fn next(&mut self) -> Option<Device> {
        while !self.entry.is_null() {
            let syspath = Path::new(unsafe {
                util::ptr_to_os_str_unchecked(ffi::udev_list_entry_get_name(self.entry))
            });

            self.entry = unsafe { ffi::udev_list_entry_get_next(self.entry) };

            match Device::from_syspath(syspath) {
                Ok(d) => return Some(d),
                Err(_) => continue,
            };
        }

        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}
