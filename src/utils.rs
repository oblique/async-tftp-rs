use async_io::Timer;
use futures_util::future::{select, Either};
use std::future::Future;
use std::io;
use std::time::Duration;

pub async fn io_timeout<T>(
    dur: Duration,
    f: impl Future<Output = io::Result<T>>,
) -> io::Result<T> {
    futures_lite::pin!(f);

    match select(f, Timer::new(dur)).await {
        Either::Left((out, _)) => out,
        Either::Right(_) => Err(io::ErrorKind::TimedOut.into()),
    }
}
