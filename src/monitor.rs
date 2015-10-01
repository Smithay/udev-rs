use std::fmt;
use std::ptr;

use std::ffi::{CString,OsStr};
use std::ops::Deref;
use std::os::unix::io::{RawFd,AsRawFd};

use ::context::{Context};
use ::device::{Device};
use ::handle::prelude::*;


/// Receives device events from the kernel.
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
    /// Creates a new `MonitorSpec`.
    pub fn new(context: &'a Context) -> ::Result<Self> {
        let name = CString::new("udev").unwrap();

        let ptr = try_alloc!(unsafe {
            ::ffi::udev_monitor_new_from_netlink(context.as_ptr(), name.as_ptr())
        });

        Ok(MonitorSpec {
            context: context,
            monitor: ptr
        })
    }

    /// Adds a filter that matches events with the given subsystem.
    pub fn match_subsystem<T: AsRef<OsStr>>(&mut self, subsystem: T) -> ::Result<()> {
        let subsystem = try!(::util::os_str_to_cstring(subsystem));

        ::util::errno_to_result(unsafe {
            ::ffi::udev_monitor_filter_add_match_subsystem_devtype(self.monitor, subsystem.as_ptr(), ptr::null())
        })
    }

    /// Adds a filter that matches events with the given subsystem and device type.
    pub fn match_subsystem_devtype<T: AsRef<OsStr>, U: AsRef<OsStr>>(&mut self, subsystem: T, devtype: U) -> ::Result<()> {
        let subsystem = try!(::util::os_str_to_cstring(subsystem));
        let devtype = try!(::util::os_str_to_cstring(devtype));

        ::util::errno_to_result(unsafe {
            ::ffi::udev_monitor_filter_add_match_subsystem_devtype(self.monitor, subsystem.as_ptr(), devtype.as_ptr())
        })
    }

    /// Adds a filter that matches events with the given tag.
    pub fn match_tag<T: AsRef<OsStr>>(&mut self, tag: T) -> ::Result<()> {
        let tag = try!(::util::os_str_to_cstring(tag));

        ::util::errno_to_result(unsafe {
            ::ffi::udev_monitor_filter_add_match_tag(self.monitor, tag.as_ptr())
        })
    }

    /// Removes all filters currently set on the monitor.
    pub fn clear_filters(&mut self) -> ::Result<()> {
        ::util::errno_to_result(unsafe {
            ::ffi::udev_monitor_filter_remove(self.monitor)
        })
    }

    /// Listens for events matching the current filters.
    pub fn listen(self) -> ::Result<Monitor<'a>> {
        try!(::util::errno_to_result(unsafe {
            ::ffi::udev_monitor_enable_receiving(self.monitor)
        }));

        Ok(Monitor { spec: self })
    }
}


/// An active monitor that can receive events.
///
/// The events received by a `Monitor` match the filters setup by the `MonitorSpec` that created
/// the `Monitor`.
///
/// Monitors are initially setup to receive events from the kernel via a nonblocking socket. A
/// variant of `poll()` should be used on the file descriptor returned by the `AsRawFd` trait to
/// wait for new events.
pub struct Monitor<'a> {
    spec: MonitorSpec<'a>
}

/// Provides raw access to the monitor's socket.
impl<'a> AsRawFd for Monitor<'a> {
    /// Returns the file descriptor of the monitor's socket.
    fn as_raw_fd(&self) -> RawFd {
        unsafe {
            ::ffi::udev_monitor_get_fd(self.spec.monitor)
        }
    }
}

impl<'a> Monitor<'a> {
    /// Receives the next available event from the monitor.
    ///
    /// This method does not block. If no events are available, it returns `None` immediately.
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

/// Types of events that can be received from udev.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum EventType {
    /// A device was added.
    Add,

    /// A device changed.
    Change,

    /// A device was removed.
    Remove,

    /// An unknown event occurred.
    Unknown,
}

impl Default for EventType {
    fn default() -> EventType {
        EventType::Unknown
    }
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


/// An event that indicates a change in device state.
pub struct MonitorEvent<'a> {
    device: Device<'a>
}

/// Provides access to the device associated with the event.
impl<'a> Deref for MonitorEvent<'a> {
    type Target = Device<'a>;

    fn deref(&self) -> &Device<'a> {
        &self.device
    }
}

impl<'a> MonitorEvent<'a> {
    /// Returns the `EventType` corresponding to this event.
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

    /// Returns the event's sequence number.
    pub fn sequence_number(&self) -> u64 {
        unsafe {
            ::ffi::udev_device_get_seqnum(self.device.as_ptr()) as u64
        }
    }

    /// Returns the device associated with this event.
    pub fn device(&self) -> &Device {
        &self.device
    }
}
