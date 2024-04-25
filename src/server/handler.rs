use futures_lite::{AsyncRead, AsyncWrite};
use std::future::Future;
use std::net::SocketAddr;
use std::path::Path;

use crate::packet;

/// Trait for implementing advance handlers.
pub trait Handler: Send {
    type Reader: AsyncRead + Unpin + Send + 'static;
    type Writer: AsyncWrite + Unpin + Send + 'static;

    /// Open `Reader` to serve a read request.
    fn read_req_open(
        &mut self,
        client: &SocketAddr,
        path: &Path,
    ) -> impl Future<Output = Result<(Self::Reader, Option<u64>), packet::Error>>
           + Send;

    /// Open `Writer` to serve a write request.
    fn write_req_open(
        &mut self,
        client: &SocketAddr,
        path: &Path,
        size: Option<u64>,
    ) -> impl Future<Output = Result<Self::Writer, packet::Error>> + Send;
}
