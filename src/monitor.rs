use std::fmt;
use std::ptr;

use std::ffi::OsStr;
use std::io::Result;
use std::ops::Deref;
use std::os::unix::io::{AsRawFd, RawFd};

#[cfg(feature = "mio")]
use mio::{event::Evented, unix::EventedFd, Poll, PollOpt, Ready, Token};

use {ffi, util};

use {AsRaw, Device, FromRaw};

/// Monitors for device events.
///
/// A monitor communicates with the kernel over a socket. Filtering events is performed efficiently
/// in the kernel, and only events that match the filters are received by the socket. Filters must
/// be setup before listening for events.
pub struct Builder {
    monitor: *mut ffi::udev_monitor,
}

impl Drop for Builder {
    fn drop(&mut self) {
        unsafe {
            ffi::udev_monitor_unref(self.monitor);
        }
    }
}

as_ffi!(Builder, monitor, ffi::udev_monitor);

impl Builder {
    /// Creates a new `Monitor`.
    pub fn new() -> Result<Self> {
        let name = b"udev\0".as_ptr() as *const libc::c_char;

        // Hack. We use this because old version libudev check udev arg by null ptr and return error
        // if udev eq nullptr. In current version first argument unused
        let ptr = try_alloc!(unsafe {
            ffi::udev_monitor_new_from_netlink([].as_mut_ptr() as *mut ffi::udev, name)
        });

        Ok(unsafe { Self::from_raw(ptr) })
    }

    /// Adds a filter that matches events for devices with the given subsystem.
    pub fn match_subsystem<T: AsRef<OsStr>>(self, subsystem: T) -> Result<Self> {
        let subsystem = util::os_str_to_cstring(subsystem)?;

        util::errno_to_result(unsafe {
            ffi::udev_monitor_filter_add_match_subsystem_devtype(
                self.monitor,
                subsystem.as_ptr(),
                ptr::null(),
            )
        })
        .and(Ok(self))
    }

    /// Adds a filter that matches events for devices with the given subsystem and device type.
    pub fn match_subsystem_devtype<T: AsRef<OsStr>, U: AsRef<OsStr>>(
        self,
        subsystem: T,
        devtype: U,
    ) -> Result<Self> {
        let subsystem = util::os_str_to_cstring(subsystem)?;
        let devtype = util::os_str_to_cstring(devtype)?;

        util::errno_to_result(unsafe {
            ffi::udev_monitor_filter_add_match_subsystem_devtype(
                self.monitor,
                subsystem.as_ptr(),
                devtype.as_ptr(),
            )
        })
        .and(Ok(self))
    }

    /// Adds a filter that matches events for devices with the given tag.
    pub fn match_tag<T: AsRef<OsStr>>(self, tag: T) -> Result<Self> {
        let tag = util::os_str_to_cstring(tag)?;

        util::errno_to_result(unsafe {
            ffi::udev_monitor_filter_add_match_tag(self.monitor, tag.as_ptr())
        })
        .and(Ok(self))
    }

    /// Removes all filters currently set on the monitor.
    pub fn clear_filters(self) -> Result<Self> {
        util::errno_to_result(unsafe { ffi::udev_monitor_filter_remove(self.monitor) })
            .and(Ok(self))
    }

    /// Listens for events matching the current filters.
    ///
    /// This method consumes the `Monitor`.
    pub fn listen(self) -> Result<Socket> {
        util::errno_to_result(unsafe { ffi::udev_monitor_enable_receiving(self.monitor) })?;

        Ok(Socket { inner: self })
    }
}

/// An active monitor that can receive events.
///
/// The events received by a `Socket` match the filters setup by the `Monitor` that created
/// the socket.
///
/// Monitors are initially setup to receive events from the kernel via a nonblocking socket. A
/// variant of `poll()` should be used on the file descriptor returned by the `AsRawFd` trait to
/// wait for new events.
pub struct Socket {
    inner: Builder,
}

impl Clone for Socket {
    fn clone(&self) -> Self {
        Self {
            inner: unsafe { Builder::from_raw(ffi::udev_monitor_ref(self.inner.monitor)) },
        }
    }
}

impl AsRaw<ffi::udev_monitor> for Socket {
    fn as_raw(&self) -> *mut ffi::udev_monitor {
        self.inner.monitor
    }

    fn into_raw(self) -> *mut ffi::udev_monitor {
        self.inner.monitor
    }
}

/// Provides raw access to the monitor's socket.
impl AsRawFd for Socket {
    /// Returns the file descriptor of the monitor's socket.
    fn as_raw_fd(&self) -> RawFd {
        unsafe { ffi::udev_monitor_get_fd(self.inner.monitor) }
    }
}

impl Iterator for Socket {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        let ptr = unsafe { ffi::udev_monitor_receive_device(self.inner.monitor) };

        if ptr.is_null() {
            None
        } else {
            let device = unsafe { Device::from_raw(ptr) };
            Some(Event { device })
        }
    }
}

/// Types of events that can be received from udev.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    /// A device was added.
    Add,

    /// A device changed.
    Change,

    /// A device was removed.
    Remove,

    /// A device was bound to driver.
    Bind,

    /// A device was unbound to driver.
    Unbind,

    /// An unknown event occurred.
    Unknown,
}

impl Default for EventType {
    fn default() -> Self {
        EventType::Unknown
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            EventType::Add => "add",
            EventType::Change => "change",
            EventType::Remove => "remove",
            EventType::Bind => "bind",
            EventType::Unbind => "unbind",
            EventType::Unknown => "unknown",
        })
    }
}

/// An event that indicates a change in device state.
pub struct Event {
    device: Device,
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
            None => None,
        };

        match value {
            Some("add") => EventType::Add,
            Some("change") => EventType::Change,
            Some("remove") => EventType::Remove,
            Some("bind") => EventType::Bind,
            Some("unbind") => EventType::Unbind,
            _ => EventType::Unknown,
        }
    }

    /// Returns the event's sequence number.
    pub fn sequence_number(&self) -> u64 {
        unsafe { ffi::udev_device_get_seqnum(self.device.as_raw()) as u64 }
    }

    /// Returns the device associated with this event.
    pub fn device(&self) -> Device {
        self.device.clone()
    }
}

#[cfg(feature = "mio")]
impl Evented for Socket {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> std::io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}
