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
    md5_ctx: Option<md5::Context>,
    md5: Option<md5::Digest>,
}

impl RandomFile {
    pub fn new(size: usize) -> Self {
        RandomFile {
            size,
            read_size: 0,
            rng: SmallRng::from_entropy(),
            md5_ctx: Some(md5::Context::new()),
            md5: None,
        }
    }

    pub fn hash(&self) -> md5::Digest {
        self.md5.expect("md5 was not computed yet").clone()
    }
}

impl Read for RandomFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.size == self.read_size {
            if let Some(md5_ctx) = self.md5_ctx.take() {
                self.md5 = Some(md5_ctx.compute());
            }

            Ok(0)
        } else {
            let len = cmp::min(buf.len(), self.size - self.read_size);

            self.rng.fill_bytes(&mut buf[..len]);
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
