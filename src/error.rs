pub use std::result::Result as StdResult;

use failure::{AsFail, Backtrace, Causes, Context, Fail};
use std::fmt;
use std::sync::Arc;

pub type Result<T> = StdResult<T, Error>;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[derive(Debug, Fail, Clone)]
pub enum ErrorKind {
    #[fail(display = "Invalid mode")]
    InvalidMode,

    #[fail(display = "Invalid packet")]
    InvalidPacket,

    #[fail(display = "IO Error: {}", _0)]
    Io(Arc<std::io::Error>),
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

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Error {
        ErrorKind::Io(Arc::new(error)).into()
    }
}
