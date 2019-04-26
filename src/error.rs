pub use std::result::Result as StdResult;

use failure::{AsFail, Backtrace, Causes, Context, Fail};
use std::fmt;

pub type Result<T> = StdResult<T, Error>;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[derive(Debug, Fail, Clone, PartialEq)]
pub enum ErrorKind {
    #[fail(display = "Invalid mode")]
    InvalidMode,

    #[fail(display = "Invalid packet")]
    InvalidPacket,

    #[fail(display = "Packet too large")]
    PacketTooLarge,
}

impl Error {
    pub fn kind(&self) -> &ErrorKind {
        self.inner.get_context()
    }

    pub fn iter_causes(&self) -> Causes {
        self.as_fail().iter_causes()
    }
}

impl Fail for Error {
    fn cause(&self) -> Option<&dyn Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Error) -> bool {
        self.kind() == other.kind()
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error {
            inner,
        }
    }
}
