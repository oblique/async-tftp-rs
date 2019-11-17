use thiserror::Error;

/// Type alias to `Result<T, Error>`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Error type of this crate.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid packet")]
    InvalidPacket,

    #[error("TFTP protocol error: {0:?}")]
    Packet(crate::packet::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to bind socket: {0}")]
    Bind(#[source] std::io::Error),

    #[error("Path '{}' is not a directory", .0.display())]
    NotDir(std::path::PathBuf),
}

impl<'a> From<nom::Err<(&'a [u8], nom::error::ErrorKind)>> for Error {
    fn from(_error: nom::Err<(&'a [u8], nom::error::ErrorKind)>) -> Error {
        Error::InvalidPacket
    }
}
