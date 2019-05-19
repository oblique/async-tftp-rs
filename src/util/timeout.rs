use futures::compat::Future01CompatExt;
use futures::future::FusedFuture;
use futures::FutureExt;
use futures_timer::Delay;
use std::future::Future;
use std::io;
use std::time::Duration;

pub fn timeout(
    dur: Duration,
) -> impl Future<Output = io::Result<()>> + FusedFuture {
    Delay::new(dur).compat().fuse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::channel::mpsc;
    use futures::{select, StreamExt};
    use std::time::Instant;

    #[runtime::test]
    async fn check() {
        let (_tx, mut rx) = mpsc::channel::<()>(1);
        let now = Instant::now();
        let mut timeout_fut = timeout(Duration::from_secs(1));

        select! {
            _ = rx.next() => panic!(),
            _ = timeout_fut => {
            },
        };

        assert_eq!(now.elapsed().as_secs(), 1);
    }
}
