extern crate libudev_sys as ffi;
extern crate libc;

use std::error;
use std::fmt;
use std::io;
use std::ptr;
use std::str;

use std::ffi::{CStr,CString,OsStr};
use std::ops::Deref;
use std::os::unix::io::{RawFd,AsRawFd};
use std::path::Path;
use std::str::FromStr;

use libc::{c_int,c_char};


macro_rules! try_alloc {
    ($exp:expr) => {{
        let ptr = $exp;

        if ptr.is_null() {
            return Err(Error::new(ErrorKind::NoMem));
        }

        ptr
    }}
}


#[derive(Debug,Clone,Copy)]
pub enum ErrorKind {
    NoMem,
    InvalidInput,
    Io(c_int)
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind
}

impl Error {
    fn new(kind: ErrorKind) -> Self {
        Error { kind: kind }
    }

    fn from_errno(errno: c_int) -> Self {
        match -errno {
            libc::ENOMEM => Error::new(ErrorKind::NoMem),
            libc::EINVAL => Error::new(ErrorKind::InvalidInput),
            n            => Error::new(ErrorKind::Io(n))
        }
    }

    fn strerror(&self) -> &str {
        let errno = match self.kind {
            ErrorKind::NoMem        => libc::ENOMEM,
            ErrorKind::InvalidInput => libc::EINVAL,
            ErrorKind::Io(errno)    => errno
        };

        unsafe {
            str::from_utf8_unchecked(CStr::from_ptr(libc::strerror(errno)).to_bytes())
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn description(&self) -> &str {
        self.strerror()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(),fmt::Error> {
        fmt.write_str(self.strerror())
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        self.strerror()
    }
}

impl From<Error> for io::Error {
    fn from(err: Error) -> io::Error {
        io::Error::new(io::ErrorKind::Other, err.strerror())
    }
}


pub struct Context {
    udev: *mut ffi::udev
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { ffi::udev_unref(self.udev) };
    }
}

impl Context {
    pub fn new() -> Result<Self,Error> {
        let ptr = try_alloc!(unsafe { ffi::udev_new() });

        Ok(Context { udev: ptr })
    }

    pub fn device_from_syspath(&self, syspath: &Path) -> Result<Device,Error> {
        let syspath = try!(util::os_str_to_cstring(syspath));

        let ptr = try_alloc!(unsafe {
            ffi::udev_device_new_from_syspath(self.udev, syspath.as_ptr())
        });

        Ok(Device {
            _context: self,
            device: ptr
        })
    }
}


pub struct MonitorSpec<'a> {
    context: &'a Context,
    monitor: *mut ffi::udev_monitor
}

impl<'a> Drop for MonitorSpec<'a> {
    fn drop(&mut self) {
        unsafe {
            ffi::udev_monitor_unref(self.monitor);
        }
    }
}

impl<'a> MonitorSpec<'a> {
    pub fn new(context: &'a Context) -> Result<Self,Error> {
        let name = CString::new("udev").unwrap();

        let ptr = try_alloc!(unsafe {
            ffi::udev_monitor_new_from_netlink(context.udev, name.as_ptr())
        });

        Ok(MonitorSpec {
            context: context,
            monitor: ptr
        })
    }

    pub fn match_subsystem<T: AsRef<OsStr>>(&mut self, subsystem: T) -> Result<(),Error> {
        let subsystem = try!(util::os_str_to_cstring(subsystem));

        util::errno_to_result(unsafe {
            ffi::udev_monitor_filter_add_match_subsystem_devtype(self.monitor, subsystem.as_ptr(), ptr::null())
        })
    }

    pub fn match_subsystem_devtype<T: AsRef<OsStr>, U: AsRef<OsStr>>(&mut self, subsystem: T, devtype: U) -> Result<(),Error> {
        let subsystem = try!(util::os_str_to_cstring(subsystem));
        let devtype = try!(util::os_str_to_cstring(devtype));

        util::errno_to_result(unsafe {
            ffi::udev_monitor_filter_add_match_subsystem_devtype(self.monitor, subsystem.as_ptr(), devtype.as_ptr())
        })
    }

    pub fn match_tag<T: AsRef<OsStr>>(&mut self, tag: T) -> Result<(),Error> {
        let tag = try!(util::os_str_to_cstring(tag));

        util::errno_to_result(unsafe {
            ffi::udev_monitor_filter_add_match_tag(self.monitor, tag.as_ptr())
        })
    }

    pub fn clear_filters(&mut self) -> Result<(),Error> {
        util::errno_to_result(unsafe {
            ffi::udev_monitor_filter_remove(self.monitor)
        })
    }

    pub fn listen(self) -> Result<Monitor<'a>,Error> {
        try!(util::errno_to_result(unsafe {
            ffi::udev_monitor_enable_receiving(self.monitor)
        }));

        Ok(Monitor { spec: self })
    }
}


pub struct Monitor<'a> {
    spec: MonitorSpec<'a>
}

impl<'a> AsRawFd for Monitor<'a> {
    fn as_raw_fd(&self) -> RawFd {
        unsafe {
            ffi::udev_monitor_get_fd(self.spec.monitor)
        }
    }
}

impl<'a> Monitor<'a> {
    pub fn receive_event(&mut self) -> Option<MonitorEvent> {
        let device = unsafe {
            ffi::udev_monitor_receive_device(self.spec.monitor)
        };

        if device.is_null() {
            None
        }
        else {
            let device = Device {
                _context: self.spec.context,
                device: device
            };

            Some(MonitorEvent { device: device })
        }
    }
}

#[derive(Debug)]
pub enum EventType {
    Add,
    Change,
    Remove,
    Unknown,
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            &EventType::Add => "add",
            &EventType::Change => "change",
            &EventType::Remove => "remove",
            &EventType::Unknown => "unknown",
        })
    }
}

pub struct MonitorEvent<'a> {
    device: Device<'a>
}

impl<'a> Deref for MonitorEvent<'a> {
    type Target = Device<'a>;

    fn deref(&self) -> &Device<'a> {
        &self.device
    }
}

impl<'a> MonitorEvent<'a> {
    pub fn event_type(&self) -> EventType {
        let value = match self.device.property_value("ACTION") {
            Some(s) => s.to_str(),
            None => None
        };

        match value {
            Some("add") => EventType::Add,
            Some("change") => EventType::Change,
            Some("remove") => EventType::Remove,
            _ => EventType::Unknown
        }
    }

    pub fn sequence_number(&self) -> u64 {
        unsafe {
            ffi::udev_device_get_seqnum(self.device.device) as u64
        }
    }

    pub fn device(&self) -> &Device {
        &self.device
    }
}


pub struct Enumerator<'a> {
    context: &'a Context,
    enumerator: *mut ffi::udev_enumerate
}

impl<'a> Drop for Enumerator<'a> {
    fn drop(&mut self) {
        unsafe { ffi::udev_enumerate_unref(self.enumerator) };
    }
}

impl<'a> Enumerator<'a> {
    pub fn new(context: &'a Context) -> Result<Self,Error> {
        let ptr = try_alloc!(unsafe { ffi::udev_enumerate_new(context.udev) });

        Ok(Enumerator {
            context: context,
            enumerator: ptr
        })
    }

    pub fn match_is_initialized(&mut self) -> Result<(),Error> {
        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_match_is_initialized(self.enumerator)
        })
    }

    pub fn match_subsystem<T: AsRef<OsStr>>(&mut self, subsystem: T) -> Result<(),Error> {
        let subsystem = try!(util::os_str_to_cstring(subsystem));

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_match_subsystem(self.enumerator, subsystem.as_ptr())
        })
    }

    pub fn match_attribute<T: AsRef<OsStr>, U: AsRef<OsStr>>(&mut self, attribute: T, value: U) -> Result<(),Error> {
        let attribute = try!(util::os_str_to_cstring(attribute));
        let value = try!(util::os_str_to_cstring(value));

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_match_sysattr(self.enumerator, attribute.as_ptr(), value.as_ptr())
        })
    }

    pub fn match_sysname<T: AsRef<OsStr>>(&mut self, sysname: T) -> Result<(),Error> {
        let sysname = try!(util::os_str_to_cstring(sysname));

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_match_sysname(self.enumerator, sysname.as_ptr())
        })
    }

    pub fn match_property<T: AsRef<OsStr>, U: AsRef<OsStr>>(&mut self, property: T, value: U) -> Result<(),Error> {
        let property = try!(util::os_str_to_cstring(property));
        let value = try!(util::os_str_to_cstring(value));

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_match_property(self.enumerator, property.as_ptr(), value.as_ptr())
        })
    }

    pub fn match_tag<T: AsRef<OsStr>>(&mut self, tag: T) -> Result<(),Error> {
        let tag = try!(util::os_str_to_cstring(tag));

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_match_tag(self.enumerator, tag.as_ptr())
        })
    }

    pub fn match_parent(&mut self, parent: &Device) -> Result<(),Error> {
        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_match_parent(self.enumerator, parent.device)
        })
    }

    pub fn nomatch_subsystem<T: AsRef<OsStr>>(&mut self, subsystem: T) -> Result<(),Error> {
        let subsystem = try!(util::os_str_to_cstring(subsystem));

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_nomatch_subsystem(self.enumerator, subsystem.as_ptr())
        })
    }

    pub fn nomatch_attribute<T: AsRef<OsStr>, U: AsRef<OsStr>>(&mut self, attribute: T, value: U) -> Result<(),Error> {
        let attribute = try!(util::os_str_to_cstring(attribute));
        let value = try!(util::os_str_to_cstring(value));

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_nomatch_sysattr(self.enumerator, attribute.as_ptr(), value.as_ptr())
        })
    }

    pub fn add_syspath(&mut self, syspath: &Path) -> Result<(),Error> {
        let syspath = try!(util::os_str_to_cstring(syspath));

        util::errno_to_result(unsafe {
            ffi::udev_enumerate_add_syspath(self.enumerator, syspath.as_ptr())
        })
    }

    pub fn scan_devices(&mut self) -> Result<Devices,Error> {
        try!(util::errno_to_result(unsafe {
            ffi::udev_enumerate_scan_devices(self.enumerator)
        }));

        Ok(Devices {
            enumerator: self,
            entry: unsafe { ffi::udev_enumerate_get_list_entry(self.enumerator) }
        })
    }
}


pub struct Devices<'a> {
    enumerator: &'a Enumerator<'a>,
    entry: *mut ffi::udev_list_entry
}

impl<'a> Iterator for Devices<'a> {
    type Item = Device<'a>;

    fn next(&mut self) -> Option<Device<'a>> {
        while !self.entry.is_null() {
            let syspath = Path::new(unsafe {
                util::ptr_to_os_str_unchecked(ffi::udev_list_entry_get_name(self.entry))
            });

            self.entry = unsafe { ffi::udev_list_entry_get_next(self.entry) };

            match self.enumerator.context.device_from_syspath(syspath) {
                Ok(d) => return Some(d),
                Err(_) => continue
            };
        }

        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}


pub struct Device<'a> {
    _context: &'a Context,
    device: *mut ffi::udev_device
}

impl<'a> Drop for Device<'a> {
    fn drop(&mut self) {
        unsafe { ffi::udev_device_unref(self.device) };
    }
}

impl<'a> Device<'a> {
    pub fn is_initialized(&self) -> bool {
        unsafe { ffi::udev_device_get_is_initialized(self.device) != 0 }
    }

    pub fn devnum(&self) -> Option<libc::dev_t> {
        match unsafe { ffi::udev_device_get_devnum(self.device) } {
            0 => None,
            n => Some(n)
        }
    }

    pub fn syspath(&self) -> &Path {
        Path::new(unsafe {
            util::ptr_to_os_str_unchecked(ffi::udev_device_get_syspath(self.device))
        })
    }

    pub fn devpath(&self) -> &OsStr {
        unsafe {
            util::ptr_to_os_str_unchecked(ffi::udev_device_get_devpath(self.device))
        }
    }

    pub fn devnode(&self) -> Option<&Path> {
        util::ptr_to_os_str(unsafe { ffi::udev_device_get_devnode(self.device) }).map(|path| {
            Path::new(path)
        })
    }

    pub fn subsystem(&self) -> &OsStr {
        unsafe {
            util::ptr_to_os_str_unchecked(ffi::udev_device_get_subsystem(self.device))
        }
    }

    pub fn sysname(&self) -> &OsStr {
        unsafe {
            util::ptr_to_os_str_unchecked(ffi::udev_device_get_sysname(self.device))
        }
    }

    pub fn sysnum(&self) -> Option<usize> {
        let ptr = unsafe { ffi::udev_device_get_sysnum(self.device) };

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
        util::ptr_to_os_str(unsafe { ffi::udev_device_get_devtype(self.device) })
    }

    pub fn driver(&self) -> Option<&OsStr> {
        util::ptr_to_os_str(unsafe { ffi::udev_device_get_driver(self.device) })
    }

    pub fn property_value<T: AsRef<OsStr>>(&self, property: T) -> Option<&OsStr> {
        let prop = match util::os_str_to_cstring(property) {
            Ok(s) => s,
            Err(_) => return None
        };

        util::ptr_to_os_str(unsafe {
            ffi::udev_device_get_property_value(self.device, prop.as_ptr())
        })
    }

    pub fn attribute_value<T: AsRef<OsStr>>(&self, attribute: T) -> Option<&OsStr> {
        let attr = match util::os_str_to_cstring(attribute) {
            Ok(s) => s,
            Err(_) => return None
        };

        util::ptr_to_os_str(unsafe {
            ffi::udev_device_get_sysattr_value(self.device, attr.as_ptr())
        })
    }

    pub fn set_attribute_value<T: AsRef<OsStr>, U: AsRef<OsStr>>(&mut self, attribute: T, value: U) -> Result<(),Error> {
        let attribute = try!(util::os_str_to_cstring(attribute));
        let value = try!(util::os_str_to_cstring(value));

        util::errno_to_result(unsafe {
            ffi::udev_device_set_sysattr_value(self.device, attribute.as_ptr(), value.as_ptr() as *mut c_char)
        })
    }

    pub fn properties(&self) -> Properties {
        Properties {
            _device: self,
            entry: unsafe { ffi::udev_device_get_properties_list_entry(self.device) }
        }
    }

    pub fn attributes(&self) -> Attributes {
        Attributes {
            device: self,
            entry: unsafe { ffi::udev_device_get_sysattr_list_entry(self.device) }
        }
    }
}


pub struct Properties<'a> {
    _device: &'a Device<'a>,
    entry: *mut ffi::udev_list_entry
}

impl<'a> Iterator for Properties<'a> {
    type Item = Property<'a>;

    fn next(&mut self) -> Option<Property<'a>> {
        if self.entry.is_null() {
            None
        }
        else {
            let name = unsafe { util::ptr_to_os_str_unchecked(ffi::udev_list_entry_get_name(self.entry)) };
            let value = unsafe { util::ptr_to_os_str_unchecked(ffi::udev_list_entry_get_value(self.entry)) };

            self.entry = unsafe { ffi::udev_list_entry_get_next(self.entry) };

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
    entry: *mut ffi::udev_list_entry
}

impl<'a> Iterator for Attributes<'a> {
    type Item = Attribute<'a>;

    fn next(&mut self) -> Option<Attribute<'a>> {
        if !self.entry.is_null() {
            let name = unsafe { util::ptr_to_os_str_unchecked(ffi::udev_list_entry_get_name(self.entry)) };

            self.entry = unsafe { ffi::udev_list_entry_get_next(self.entry) };

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


mod util {
    use libc;
    use std::slice;

    use std::ffi::{CString,OsStr};

    use libc::{c_int,c_char};

    use std::os::unix::prelude::*;

    use super::{Error,ErrorKind};

    pub fn ptr_to_os_str<'a>(ptr: *const c_char) -> Option<&'a OsStr> {
        if !ptr.is_null() {
            Some(unsafe { ptr_to_os_str_unchecked(ptr) })
        }
        else {
            None
        }
    }

    pub unsafe fn ptr_to_os_str_unchecked<'a>(ptr: *const c_char) -> &'a OsStr {
        OsStr::from_bytes(slice::from_raw_parts(ptr as *const u8, libc::strlen(ptr) as usize))
    }

    pub fn os_str_to_cstring<T: AsRef<OsStr>>(s: T) -> Result<CString,Error> {
        match CString::new(s.as_ref().as_bytes()) {
            Ok(s) => Ok(s),
            Err(_) => return Err(Error::new(ErrorKind::InvalidInput))
        }
    }

    pub fn errno_to_result(errno: c_int) -> Result<(),Error> {
        match errno {
            0 => Ok(()),
            e => Err(Error::from_errno(e))
        }
    }
}
