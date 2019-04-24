pub use std::result::Result as StdResult;

use failure::{AsFail, Backtrace, Causes, Context, Fail};
use std::fmt;
use std::sync::Arc;

pub type Result<T> = StdResult<T, Error>;

#[derive(Debug, Clone)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[derive(Debug, Fail, Clone, PartialEq)]
pub enum ErrorKind {
    #[fail(display = "Decode error: {}", _0)]
    DecodeError(&'static str),
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
            inner: Arc::new(Context::new(kind)),
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(context: Context<ErrorKind>) -> Error {
        Error {
            inner: Arc::new(context),
        }
    }
}

impl From<Arc<Context<ErrorKind>>> for Error {
    fn from(inner: Arc<Context<ErrorKind>>) -> Error {
        Error {
            inner,
        }
    }
}
