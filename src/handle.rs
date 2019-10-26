use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite};
use std::net::SocketAddr;
use std::path::Path;

use crate::TftpError;

#[async_trait]
pub trait Handle: Send {
    type Reader: AsyncRead + Unpin + Send + 'static;
    type Writer: AsyncWrite + Unpin + Send + 'static;

    async fn read_req_open(
        &mut self,
        client: &SocketAddr,
        path: &Path,
    ) -> Result<(Self::Reader, Option<u64>), TftpError>;

    async fn read_req_served(
        &mut self,
        _client: &SocketAddr,
        _path: &Path,
        _reader: Self::Reader,
    ) {
    }

    async fn write_open(
        &mut self,
        client: &SocketAddr,
        path: &Path,
        size: Option<u64>,
    ) -> Result<Self::Writer, TftpError>;
}
