use futures::io::AsyncRead;
use rand::rngs::SmallRng;
use rand::{RngCore, SeedableRng};
use std::cmp;
use std::io::{self, Read};
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct RandomFile {
    size: usize,
    read_size: usize,
    rng: SmallRng,
    md5_ctx: md5::Context,
}

impl RandomFile {
    pub fn new(size: usize) -> Self {
        RandomFile {
            size,
            read_size: 0,
            rng: SmallRng::from_entropy(),
            md5_ctx: md5::Context::new(),
        }
    }

    pub fn hash(self) -> md5::Digest {
        self.md5_ctx.compute()
    }
}

impl Read for RandomFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.size == self.read_size {
            Ok(0)
        } else {
            let len = cmp::min(buf.len(), self.size - self.read_size);

            self.rng.fill_bytes(&mut buf[..len]);
            self.md5_ctx.consume(&buf[..len]);
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
