use std::fmt;
use std::ptr;

use std::ffi::{CString,OsStr};
use std::ops::Deref;
use std::os::unix::io::{RawFd,AsRawFd};

use ::context::{Context};
use ::device::{Device};
use ::error::{Error};
use ::handle::prelude::*;

pub struct MonitorSpec<'a> {
    context: &'a Context,
    monitor: *mut ::ffi::udev_monitor
}

impl<'a> Drop for MonitorSpec<'a> {
    fn drop(&mut self) {
        unsafe {
            ::ffi::udev_monitor_unref(self.monitor);
        }
    }
}

impl<'a> MonitorSpec<'a> {
    pub fn new(context: &'a Context) -> Result<Self,Error> {
        let name = CString::new("udev").unwrap();

        let ptr = try_alloc!(unsafe {
            ::ffi::udev_monitor_new_from_netlink(context.as_ptr(), name.as_ptr())
        });

        Ok(MonitorSpec {
            context: context,
            monitor: ptr
        })
    }

    pub fn match_subsystem<T: AsRef<OsStr>>(&mut self, subsystem: T) -> Result<(),Error> {
        let subsystem = try!(::util::os_str_to_cstring(subsystem));

        ::util::errno_to_result(unsafe {
            ::ffi::udev_monitor_filter_add_match_subsystem_devtype(self.monitor, subsystem.as_ptr(), ptr::null())
        })
    }

    pub fn match_subsystem_devtype<T: AsRef<OsStr>, U: AsRef<OsStr>>(&mut self, subsystem: T, devtype: U) -> Result<(),Error> {
        let subsystem = try!(::util::os_str_to_cstring(subsystem));
        let devtype = try!(::util::os_str_to_cstring(devtype));

        ::util::errno_to_result(unsafe {
            ::ffi::udev_monitor_filter_add_match_subsystem_devtype(self.monitor, subsystem.as_ptr(), devtype.as_ptr())
        })
    }

    pub fn match_tag<T: AsRef<OsStr>>(&mut self, tag: T) -> Result<(),Error> {
        let tag = try!(::util::os_str_to_cstring(tag));

        ::util::errno_to_result(unsafe {
            ::ffi::udev_monitor_filter_add_match_tag(self.monitor, tag.as_ptr())
        })
    }

    pub fn clear_filters(&mut self) -> Result<(),Error> {
        ::util::errno_to_result(unsafe {
            ::ffi::udev_monitor_filter_remove(self.monitor)
        })
    }

    pub fn listen(self) -> Result<Monitor<'a>,Error> {
        try!(::util::errno_to_result(unsafe {
            ::ffi::udev_monitor_enable_receiving(self.monitor)
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
            ::ffi::udev_monitor_get_fd(self.spec.monitor)
        }
    }
}

impl<'a> Monitor<'a> {
    pub fn receive_event<'b>(&'b mut self) -> Option<MonitorEvent<'a>> {
        let device = unsafe {
            ::ffi::udev_monitor_receive_device(self.spec.monitor)
        };

        if device.is_null() {
            None
        }
        else {
            let device = ::device::new(self.spec.context, device);

            Some(MonitorEvent { device: device })
        }
    }
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum EventType {
    Add,
    Change,
    Remove,
    Unknown,
}

impl Default for EventType {
    fn default() -> EventType { EventType::Unknown }
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
            ::ffi::udev_device_get_seqnum(self.device.as_ptr()) as u64
        }
    }

    pub fn device(&self) -> &Device {
        &self.device
    }
}
