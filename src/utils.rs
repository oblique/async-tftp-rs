use futures::future::{select, Either};
use smol::Timer;
use std::future::Future;
use std::io;
use std::time::Duration;

pub async fn io_timeout<T>(
    dur: Duration,
    f: impl Future<Output = io::Result<T>>,
) -> io::Result<T> {
    futures::pin_mut!(f);

    match select(f, Timer::after(dur)).await {
        Either::Left((out, _)) => out,
        Either::Right(_) => Err(io::ErrorKind::TimedOut.into()),
    }
}
