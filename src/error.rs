use std::ffi::CStr;
use std::fmt;
use std::io;
use std::str;

use std::error::Error as StdError;
use std::result::Result as StdResult;

use ::libc::c_int;

/// A `Result` type for libudev operations.
pub type Result<T> = StdResult<T,Error>;

/// Types of errors that occur in libudev.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ErrorKind {
    NoMem,
    InvalidInput,
    Io(c_int)
}

/// The error type for libudev operations.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind
}

impl Error {
    fn strerror(&self) -> &str {
        let errno = match self.kind {
            ErrorKind::NoMem        => ::libc::ENOMEM,
            ErrorKind::InvalidInput => ::libc::EINVAL,
            ErrorKind::Io(errno)    => errno
        };

        unsafe {
            str::from_utf8_unchecked(CStr::from_ptr(::libc::strerror(errno)).to_bytes())
        }
    }

    /// Returns the corresponding `ErrorKind` for this error.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns a description of the error.
    pub fn description(&self) -> &str {
        self.strerror()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> StdResult<(),fmt::Error> {
        fmt.write_str(self.strerror())
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        self.strerror()
    }
}

impl From<Error> for io::Error {
    fn from(err: Error) -> io::Error {
        io::Error::new(io::ErrorKind::Other, err.strerror())
    }
}

pub fn new(kind: ErrorKind) -> Error {
    Error { kind: kind }
}

pub fn from_errno(errno: c_int) -> Error {
    match -errno {
        ::libc::ENOMEM => Error { kind: ErrorKind::NoMem },
        ::libc::EINVAL => Error { kind: ErrorKind::InvalidInput },
        n              => Error { kind: ErrorKind::Io(n) }
    }
}
