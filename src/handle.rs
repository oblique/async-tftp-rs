use futures::AsyncRead;

use crate::error::Result;

pub trait ReadHandle: Send {
    type Reader: AsyncRead + Unpin + Send + 'static;

    fn open(&mut self, path: &str) -> Result<(Self::Reader, Option<u64>)>;
}
