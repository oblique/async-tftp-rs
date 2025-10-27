use crate::Error;
use crate::Result;
use async_io::Timer;
use futures_lite::future;
use std::future::Future;
use std::io::ErrorKind;
use std::time::Duration;

pub async fn io_timeout<T>(
    dur: Duration,
    f: impl Future<Output = Result<T>>,
) -> Result<T> {
    future::race(f, async move {
        Timer::after(dur).await;
        Err(Error::Io(ErrorKind::TimedOut.into()))
    })
    .await
}
