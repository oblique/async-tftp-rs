use async_io::Timer;
use futures_lite::future;
use std::future::Future;
use std::io;
use std::time::Duration;

pub async fn io_timeout<T>(
    dur: Duration,
    f: impl Future<Output = io::Result<T>>,
) -> io::Result<T> {
    future::race(f, async move {
        Timer::after(dur).await;
        Err(io::ErrorKind::TimedOut.into())
    })
    .await
}
