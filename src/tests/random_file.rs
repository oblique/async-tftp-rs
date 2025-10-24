#![cfg(feature = "external-client-tests")]
#![cfg(target_os = "linux")]

use async_channel::Sender;
use futures_lite::AsyncRead;
use rand::RngCore;
use std::cmp;
use std::io::{self, Read};
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct RandomFile {
    size: usize,
    read_size: usize,
    md5_ctx: Option<md5::Context>,
    md5_tx: Option<Sender<md5::Digest>>,
}

impl RandomFile {
    pub fn new(size: usize, md5_tx: Sender<md5::Digest>) -> Self {
        RandomFile {
            size,
            read_size: 0,
            md5_ctx: Some(md5::Context::new()),
            md5_tx: Some(md5_tx),
        }
    }
}

impl Read for RandomFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.size == self.read_size {
            if let (Some(md5_ctx), Some(md5_tx)) =
                (self.md5_ctx.take(), self.md5_tx.take())
            {
                md5_tx
                    .try_send(md5_ctx.finalize())
                    .expect("failed to send md5 digest");
            }

            Ok(0)
        } else {
            let len = cmp::min(buf.len(), self.size - self.read_size);

            rand::rng().fill_bytes(&mut buf[..len]);
            self.md5_ctx.as_mut().unwrap().consume(&buf[..len]);
            self.read_size += len;

            Ok(len)
        }
    }
}

impl AsyncRead for RandomFile {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<Result<usize, io::Error>> {
        Poll::Ready(self.read(buf))
    }
}
