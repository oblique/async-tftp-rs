use async_io::Timer;
use futures_util::future;
use pin_utils::pin_mut;
use std::future::Future;
use std::io;
use std::time::Duration;

pub async fn io_timeout<T>(
    dur: Duration,
    f: impl Future<Output = io::Result<T>>,
) -> io::Result<T> {
    let timer = async move {
        Timer::after(dur).await;
        Err(io::ErrorKind::TimedOut.into())
    };

    pin_mut!(f);
    pin_mut!(timer);

    future::select(f, timer).await.factor_first().0
}
