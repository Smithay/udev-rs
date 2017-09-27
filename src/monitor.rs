use std::fmt;
use std::ptr;

use std::ffi::{CString, OsStr};
use std::ops::Deref;
use std::os::unix::io::{RawFd, AsRawFd};

use ::{AsRaw, Context, Device, FromRawWithContext};


/// Monitors for device events.
///
/// A monitor communicates with the kernel over a socket. Filtering events is performed efficiently
/// in the kernel, and only events that match the filters are received by the socket. Filters must
/// be setup before listening for events.
pub struct MonitorBuilder {
    monitor: *mut ::ffi::udev_monitor,
    context: Context,
}

impl Drop for MonitorBuilder {
    fn drop(&mut self) {
        unsafe {
            ::ffi::udev_monitor_unref(self.monitor);
        }
    }
}

as_ffi!(MonitorBuilder, monitor, ::ffi::udev_monitor);

impl FromRawWithContext<::ffi::udev_monitor> for MonitorBuilder {
    unsafe fn from_raw(context: &Context, ptr: *mut ::ffi::udev_monitor) -> MonitorBuilder {
        MonitorBuilder {
            monitor: ptr,
            context: context.clone(),
        }
    }
}

impl MonitorBuilder {
    /// Creates a new `Monitor`.
    pub fn new(context: &Context) -> ::Result<Self> {
        let name = CString::new("udev").unwrap();

        let ptr = try_alloc!(unsafe {
            ::ffi::udev_monitor_new_from_netlink(context.as_raw(), name.as_ptr())
        });

        Ok(unsafe { MonitorBuilder::from_raw(context, ptr) })
    }

    /// Adds a filter that matches events for devices with the given subsystem.
    pub fn match_subsystem<T: AsRef<OsStr>>(&mut self, subsystem: T) -> ::Result<()> {
        let subsystem = try!(::util::os_str_to_cstring(subsystem));

        ::util::errno_to_result(unsafe {
            ::ffi::udev_monitor_filter_add_match_subsystem_devtype(self.monitor, subsystem.as_ptr(), ptr::null())
        })
    }

    /// Adds a filter that matches events for devices with the given subsystem and device type.
    pub fn match_subsystem_devtype<T: AsRef<OsStr>, U: AsRef<OsStr>>(&mut self, subsystem: T, devtype: U) -> ::Result<()> {
        let subsystem = try!(::util::os_str_to_cstring(subsystem));
        let devtype = try!(::util::os_str_to_cstring(devtype));

        ::util::errno_to_result(unsafe {
            ::ffi::udev_monitor_filter_add_match_subsystem_devtype(self.monitor, subsystem.as_ptr(), devtype.as_ptr())
        })
    }

    /// Adds a filter that matches events for devices with the given tag.
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
    ///
    /// This method consumes the `Monitor`.
    pub fn listen(self) -> ::Result<MonitorSocket> {
        try!(::util::errno_to_result(unsafe {
            ::ffi::udev_monitor_enable_receiving(self.monitor)
        }));

        Ok(MonitorSocket { inner: self })
    }
}


/// An active monitor that can receive events.
///
/// The events received by a `MonitorSocket` match the filters setup by the `Monitor` that created
/// the socket.
///
/// Monitors are initially setup to receive events from the kernel via a nonblocking socket. A
/// variant of `poll()` should be used on the file descriptor returned by the `AsRawFd` trait to
/// wait for new events.
pub struct MonitorSocket {
    inner: MonitorBuilder,
}

impl Clone for MonitorSocket {
    fn clone(&self) -> MonitorSocket {
        MonitorSocket {
            inner: unsafe { MonitorBuilder::from_raw(&self.inner.context, ::ffi::udev_monitor_ref(self.inner.monitor)) },
        }
    }
}

impl AsRaw<::ffi::udev_monitor> for MonitorSocket {
    fn as_raw(&self) -> *mut ::ffi::udev_monitor {
        self.inner.monitor
    }

    fn into_raw(self) -> *mut ::ffi::udev_monitor {
        self.inner.monitor
    }
}

impl FromRawWithContext<::ffi::udev_monitor> for MonitorSocket {
    unsafe fn from_raw(context: &Context, ptr: *mut ::ffi::udev_monitor) -> MonitorSocket {
        MonitorSocket {
            inner: MonitorBuilder::from_raw(context, ptr),
        }
    }
}
/// Provides raw access to the monitor's socket.
impl AsRawFd for MonitorSocket {
    /// Returns the file descriptor of the monitor's socket.
    fn as_raw_fd(&self) -> RawFd {
        unsafe {
            ::ffi::udev_monitor_get_fd(self.inner.monitor)
        }
    }
}

impl Iterator for MonitorSocket {
    type Item = Event;
    
    fn next(&mut self) -> Option<Event> {
        let ptr = unsafe {
            ::ffi::udev_monitor_receive_device(self.inner.monitor)
        };

        if ptr.is_null() {
            None
        } else {
            let device = unsafe { ::Device::from_raw(&self.inner.context, ptr) };
            Some(Event { device: device })
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
pub struct Event {
    device: Device
}

/// Provides access to the device associated with the event.
impl Deref for Event {
    type Target = Device;

    fn deref(&self) -> &Device {
        &self.device
    }
}

impl Event {
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
            ::ffi::udev_device_get_seqnum(self.device.as_raw()) as u64
        }
    }

    /// Returns the device associated with this event.
    pub fn device(&self) -> Device {
        self.device.clone()
    }
}
