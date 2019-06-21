use failure::Fail;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Invalid mode")]
    InvalidMode,

    #[fail(display = "Invalid packet")]
    InvalidPacket,

    #[fail(display = "Invalid operation")]
    InvalidOperation,

    #[fail(display = "IO Error: {}", _0)]
    Io(std::io::Error),

    #[fail(display = "Failed to bind socket: {}", _0)]
    Bind(std::io::Error),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Error {
        Error::Io(error)
    }
}
