use futures::prelude::*;
use futures::sync::mpsc::{spawn, SpawnHandle};
use std::io::Read;
use tokio::runtime::TaskExecutor;

use crate::error::*;

pub struct BlockStream {
    worker: SpawnHandle<Vec<u8>, Error>,
}

impl BlockStream {
    pub fn new<R>(reader: R, block_size: usize, executor: TaskExecutor) -> Self
    where
        R: Read + Send + 'static,
    {
        let inner = Inner {
            reader,
            block_size,
            eof: false,
        };

        let worker = spawn(inner, &executor, 10);

        BlockStream {
            worker,
        }
    }
}

impl Stream for BlockStream {
    type Item = Vec<u8>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.worker.poll()
    }
}

struct Inner<R: Read> {
    reader: R,
    block_size: usize,
    eof: bool,
}

impl<R> Stream for Inner<R>
where
    R: Read,
{
    type Item = Vec<u8>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if self.eof {
            return Ok(Async::Ready(None));
        }

        let mut buf = vec![0; self.block_size];
        let mut buf_len = 0;

        while buf_len < self.block_size {
            let len = self.reader.read(&mut buf[buf_len..])?;

            if len == 0 {
                self.eof = true;
                break;
            }

            buf_len += len;
        }

        buf.resize(buf_len, 0);
        Ok(Async::Ready(Some(buf)))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::BytesMut;
    use futures::future;
    use rand::{thread_rng, Rng};
    use std::cmp::min;
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};
    use tokio::runtime::Runtime;

    #[test]
    fn check() {
        let mut buf = BytesMut::new();

        buf.resize(512 * 10 + 123, 0);
        thread_rng().fill(&mut buf[..]);

        let mut runtime = Runtime::new().unwrap();
        let stream =
            BlockStream::new(Cursor::new(buf.clone()), 512, runtime.executor());

        let count = Arc::new(Mutex::new(0));
        let count_clone = count.clone();

        let fut = stream
            .for_each(move |x| {
                let len = min(512, buf.len());

                assert_eq!(&x[..], &buf[..len]);
                buf.advance(len);
                *count_clone.lock().unwrap() += len;

                future::ok(())
            })
            .into_future();

        runtime.block_on(fut).unwrap();
        assert_eq!(*count.lock().unwrap(), 512 * 10 + 123);
    }
}
