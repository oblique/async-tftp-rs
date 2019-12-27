use std::net::SocketAddr;
use std::path::Path;

use crate::packet;
use crate::runtime::{AsyncRead, AsyncWrite};

/// Trait for implementing advance handlers.
#[crate::async_trait]
pub trait Handler: Send {
    type Reader: AsyncRead + Unpin + Send + 'static;
    type Writer: AsyncWrite + Unpin + Send + 'static;

    /// Open `Reader` to serve a read request.
    async fn read_req_open(
        &mut self,
        client: &SocketAddr,
        path: &Path,
    ) -> Result<(Self::Reader, Option<u64>), packet::Error>;

    async fn write_req_open(
        &mut self,
        client: &SocketAddr,
        path: &Path,
        size: Option<u64>,
    ) -> Result<Self::Writer, packet::Error>;
}
