use crate::packet;
use crate::server::Handler;
use futures_lite::io::Sink;
use futures_lite::AsyncRead;
use std::net::SocketAddr;
use std::path::Path;

pub struct ReaderHandler<Reader> {
    reader: Option<Reader>,
    size: Option<u64>,
}

impl<Reader> ReaderHandler<Reader> {
    pub fn new(reader: Reader, size: Option<u64>) -> Self {
        ReaderHandler {
            reader: Some(reader),
            size,
        }
    }
}

impl<Reader: Send + AsyncRead + Unpin + 'static> Handler
    for ReaderHandler<Reader>
{
    type Reader = Reader;
    type Writer = Sink;

    async fn read_req_open(
        &mut self,
        _client: &SocketAddr,
        _path: &Path,
    ) -> Result<(Self::Reader, Option<u64>), packet::Error> {
        Ok((self.reader.take().expect("reader already consumed"), self.size))
    }

    async fn write_req_open(
        &mut self,
        _client: &SocketAddr,
        _path: &Path,
        _size: Option<u64>,
    ) -> Result<Self::Writer, packet::Error> {
        Err(packet::Error::IllegalOperation)
    }
}
