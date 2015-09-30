use std::str;

use std::ffi::{CStr,OsStr};
use std::path::Path;
use std::str::FromStr;

use libc::{c_char,dev_t};

use ::error::Error;
use ::Context;

use ::handle::*;

pub fn new(context: &Context, device: *mut ::ffi::udev_device) -> Device {
    Device {
        _context: context,
        device: device,
    }
}

pub struct Device<'a> {
    _context: &'a Context,
    device: *mut ::ffi::udev_device,
}

impl<'a> Drop for Device<'a> {
    fn drop(&mut self) {
        unsafe { ::ffi::udev_device_unref(self.device) };
    }
}

impl<'a> Handle<::ffi::udev_device> for Device<'a> {
    fn as_ptr(&self) -> *mut ::ffi::udev_device {
        self.device
    }
}

impl<'a> Device<'a> {
    pub fn is_initialized(&self) -> bool {
        unsafe { ::ffi::udev_device_get_is_initialized(self.device) != 0 }
    }

    pub fn devnum(&self) -> Option<dev_t> {
        match unsafe { ::ffi::udev_device_get_devnum(self.device) } {
            0 => None,
            n => Some(n)
        }
    }

    pub fn syspath(&self) -> &Path {
        Path::new(unsafe {
            ::util::ptr_to_os_str_unchecked(::ffi::udev_device_get_syspath(self.device))
        })
    }

    pub fn devpath(&self) -> &OsStr {
        unsafe {
            ::util::ptr_to_os_str_unchecked(::ffi::udev_device_get_devpath(self.device))
        }
    }

    pub fn devnode(&self) -> Option<&Path> {
        ::util::ptr_to_os_str(unsafe { ::ffi::udev_device_get_devnode(self.device) }).map(|path| {
            Path::new(path)
        })
    }

    pub fn subsystem(&self) -> &OsStr {
        unsafe {
            ::util::ptr_to_os_str_unchecked(::ffi::udev_device_get_subsystem(self.device))
        }
    }

    pub fn sysname(&self) -> &OsStr {
        unsafe {
            ::util::ptr_to_os_str_unchecked(::ffi::udev_device_get_sysname(self.device))
        }
    }

    pub fn sysnum(&self) -> Option<usize> {
        let ptr = unsafe { ::ffi::udev_device_get_sysnum(self.device) };

        if !ptr.is_null() {
            match str::from_utf8(unsafe { CStr::from_ptr(ptr) }.to_bytes()) {
                Err(_) => None,
                Ok(s) => FromStr::from_str(s).ok()
            }
        }
        else {
            None
        }
    }

    pub fn devtype(&self) -> Option<&OsStr> {
        ::util::ptr_to_os_str(unsafe { ::ffi::udev_device_get_devtype(self.device) })
    }

    pub fn driver(&self) -> Option<&OsStr> {
        ::util::ptr_to_os_str(unsafe { ::ffi::udev_device_get_driver(self.device) })
    }

    pub fn property_value<T: AsRef<OsStr>>(&self, property: T) -> Option<&OsStr> {
        let prop = match ::util::os_str_to_cstring(property) {
            Ok(s) => s,
            Err(_) => return None
        };

        ::util::ptr_to_os_str(unsafe {
            ::ffi::udev_device_get_property_value(self.device, prop.as_ptr())
        })
    }

    pub fn attribute_value<T: AsRef<OsStr>>(&self, attribute: T) -> Option<&OsStr> {
        let attr = match ::util::os_str_to_cstring(attribute) {
            Ok(s) => s,
            Err(_) => return None
        };

        ::util::ptr_to_os_str(unsafe {
            ::ffi::udev_device_get_sysattr_value(self.device, attr.as_ptr())
        })
    }

    pub fn set_attribute_value<T: AsRef<OsStr>, U: AsRef<OsStr>>(&mut self, attribute: T, value: U) -> Result<(),Error> {
        let attribute = try!(::util::os_str_to_cstring(attribute));
        let value = try!(::util::os_str_to_cstring(value));

        ::util::errno_to_result(unsafe {
            ::ffi::udev_device_set_sysattr_value(self.device, attribute.as_ptr(), value.as_ptr() as *mut c_char)
        })
    }

    pub fn properties(&self) -> Properties {
        Properties {
            _device: self,
            entry: unsafe { ::ffi::udev_device_get_properties_list_entry(self.device) }
        }
    }

    pub fn attributes(&self) -> Attributes {
        Attributes {
            device: self,
            entry: unsafe { ::ffi::udev_device_get_sysattr_list_entry(self.device) }
        }
    }
}


pub struct Properties<'a> {
    _device: &'a Device<'a>,
    entry: *mut ::ffi::udev_list_entry
}

impl<'a> Iterator for Properties<'a> {
    type Item = Property<'a>;

    fn next(&mut self) -> Option<Property<'a>> {
        if self.entry.is_null() {
            None
        }
        else {
            let name = unsafe { ::util::ptr_to_os_str_unchecked(::ffi::udev_list_entry_get_name(self.entry)) };
            let value = unsafe { ::util::ptr_to_os_str_unchecked(::ffi::udev_list_entry_get_value(self.entry)) };

            self.entry = unsafe { ::ffi::udev_list_entry_get_next(self.entry) };

            Some(Property {
                name: name,
                value: value
            })
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}


pub struct Property<'a> {
    name: &'a OsStr,
    value: &'a OsStr
}

impl<'a> Property<'a> {
    pub fn name(&self) -> &OsStr {
        self.name
    }

    pub fn value(&self) -> &OsStr {
        self.value
    }
}


pub struct Attributes<'a> {
    device: &'a Device<'a>,
    entry: *mut ::ffi::udev_list_entry
}

impl<'a> Iterator for Attributes<'a> {
    type Item = Attribute<'a>;

    fn next(&mut self) -> Option<Attribute<'a>> {
        if !self.entry.is_null() {
            let name = unsafe { ::util::ptr_to_os_str_unchecked(::ffi::udev_list_entry_get_name(self.entry)) };

            self.entry = unsafe { ::ffi::udev_list_entry_get_next(self.entry) };

            Some(Attribute {
                device: self.device,
                name: name
            })
        }
        else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}


pub struct Attribute<'a> {
    device: &'a Device<'a>,
    name: &'a OsStr
}

impl<'a> Attribute<'a> {
    pub fn name(&self) -> &OsStr {
        self.name
    }

    pub fn value(&self) -> Option<&OsStr> {
        self.device.attribute_value(self.name)
    }
}
