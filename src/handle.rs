use futures::{AsyncRead, AsyncWrite};

use crate::TftpError;

pub trait Handle: Send {
    type Reader: AsyncRead + Unpin + Send + 'static;
    type Writer: AsyncWrite + Unpin + Send + 'static;

    fn read_open(
        &mut self,
        path: &str,
    ) -> Result<(Self::Reader, Option<u64>), TftpError>;

    fn write_open(
        &mut self,
        path: &str,
        size: Option<u64>,
    ) -> Result<Self::Writer, TftpError>;
}
