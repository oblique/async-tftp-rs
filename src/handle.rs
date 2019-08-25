use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite};

use crate::TftpError;

#[async_trait]
pub trait Handle: Send {
    type Reader: AsyncRead + Unpin + Send + 'static;
    type Writer: AsyncWrite + Unpin + Send + 'static;

    async fn read_open(
        &mut self,
        path: &str,
    ) -> Result<(Self::Reader, Option<u64>), TftpError>;

    async fn write_open(
        &mut self,
        path: &str,
        size: Option<u64>,
    ) -> Result<Self::Writer, TftpError>;
}
