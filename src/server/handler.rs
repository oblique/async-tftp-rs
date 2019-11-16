use futures::AsyncRead;
#[cfg(feature = "unstable")]
use futures::AsyncWrite;
use std::net::SocketAddr;
use std::path::Path;

use crate::packet;

#[crate::async_trait]
pub trait Handler: Send {
    type Reader: AsyncRead + Unpin + Send + 'static;
    #[cfg(feature = "unstable")]
    type Writer: AsyncWrite + Unpin + Send + 'static;

    async fn read_req_open(
        &mut self,
        client: &SocketAddr,
        path: &Path,
    ) -> Result<(Self::Reader, Option<u64>), packet::Error>;

    #[cfg(feature = "unstable")]
    async fn read_req_served(
        &mut self,
        _client: &SocketAddr,
        _path: &Path,
        _reader: Self::Reader,
    ) {
    }

    #[cfg(feature = "unstable")]
    async fn write_open(
        &mut self,
        client: &SocketAddr,
        path: &Path,
        size: Option<u64>,
    ) -> Result<Self::Writer, packet::Error>;
}
