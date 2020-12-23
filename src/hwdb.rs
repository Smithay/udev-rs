use std::ffi::{CString, OsStr};
use std::io::Result;
use std::marker::PhantomData;
use std::os::unix::ffi::OsStrExt;

use libc::c_char;

use AsRaw;
use ffi;
use list::List;
use FromRaw;
use udev::Udev;

/// Rust wrapper for the `udev_hwdb` struct, which provides access to `udev`'s
/// hardware database API.
///
/// Like the `udev` struct, `udev_hwdb` is refcounted and automatically managed
/// by the Rust wrapper.
pub struct Hwdb {
    hwdb: *mut ffi::udev_hwdb,
}

impl Clone for Hwdb {
    fn clone(&self) -> Self {
        unsafe { Self::from_raw(ffi::udev_hwdb_ref(self.hwdb)) }
    }
}

impl Drop for Hwdb {
    fn drop(&mut self) {
        unsafe { ffi::udev_hwdb_unref(self.hwdb) };
    }
}

as_ffi!(Hwdb, hwdb, ffi::udev_hwdb, ffi::udev_hwdb_ref);

impl Hwdb {
    /// Creates a new Hwdb context.
    pub fn new(udev: &Udev) -> Result<Self> {
        // NOTE: udev_hwdb_new states that its first parameter is unused.
        // However, older versions of udev check it against NULL, so we can't just pass an
        // empty pointer in. Consequently, we borrow a `Udev`'s underlying pointer
        // without doing any further reference/lifetime management.
        let ptr = try_alloc!(unsafe { ffi::udev_hwdb_new(udev.as_raw()) });
        Ok(unsafe { Self::from_raw(ptr) })
    }

    /// Queries the hardware database with the given `modalias` query,
    /// returning an iterator over each matching entry.
    pub fn query<S: AsRef<OsStr>>(&self, modalias: S) -> List<Hwdb> {
        // NOTE: This unwrap can fail if someone passes a string that contains an internal NUL.
        let modalias = CString::new(modalias.as_ref().as_bytes()).unwrap();
        List {
            entry: unsafe {
                ffi::udev_hwdb_get_properties_list_entry(
                    self.hwdb,
                    modalias.as_ptr() as *const c_char,
                    0,
                )
            },
            phantom: PhantomData,
        }
    }

    /// Returns the first entry value with the given name, or `None` if no result exists.
    pub fn query_one<'a, S: AsRef<OsStr>>(&'a self, modalias: S, name: S) -> Option<&'a OsStr> {
        self.query(modalias).find(|e| e.name == name.as_ref()).map(|e| e.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query() {
        let udev = Udev::new().unwrap();
        let hwdb = Hwdb::new(&udev).unwrap();
        // Query the hwdb for a device that should always be known:
        // the Linux Foundation's USB 1.1. root hub
        let results: Vec<_> = hwdb.query("usb:v1D6Bp0001").collect();

        assert_eq!(results.len(), 2);

        // We expect an ID_VENDOR_FROM_DATABASE and an ID_MODEL_FROM_DATABASE with corresponding
        // values; no order is specified by udev.

        assert!(results.iter().find(|e| e.name == "ID_VENDOR_FROM_DATABASE").is_some());
        assert!(results.iter().find(|e| e.name == "ID_MODEL_FROM_DATABASE").is_some());

        assert!(results.iter().find(|e| e.value == "Linux Foundation").is_some());
        assert!(results.iter().find(|e| e.value == "1.1 root hub").is_some());
    }

    #[test]
    fn test_query_one() {
        let udev = Udev::new().unwrap();
        let hwdb = Hwdb::new(&udev).unwrap();
        let value = hwdb.query_one("usb:v1D6Bp0001", "ID_MODEL_FROM_DATABASE").unwrap();

        assert_eq!(value, "1.1 root hub");
    }
}
