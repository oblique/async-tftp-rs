use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

#[cfg(feature = "use-async-std")]
pub(crate) use async_std::{fs::File, net::SocketAddr, sync::Mutex};
#[cfg(feature = "use-async-std")]
use async_std::{io, net, task};
#[cfg(feature = "use-async-std")]
pub(crate) use futures::io::{AsyncRead, AsyncReadExt};
#[cfg(all(feature = "use-async-std", feature = "unstable"))]
pub(crate) use futures::io::{AsyncWrite, AsyncWriteExt, Sink};

#[cfg(feature = "use-tokio")]
pub(crate) use std::net::SocketAddr;
#[cfg(all(feature = "use-tokio", feature = "unstable"))]
pub(crate) use tokio::io::{AsyncWrite, AsyncWriteExt, Sink};
#[cfg(feature = "use-tokio")]
pub(crate) use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt},
    sync::Mutex,
};
#[cfg(feature = "use-tokio")]
use tokio::{io, net, task};

/// Wrap of UdpSocket
pub(crate) struct UdpSocket {
    inner: net::UdpSocket,
}

impl UdpSocket {
    pub(crate) async fn bind(addr: SocketAddr) -> io::Result<UdpSocket> {
        net::UdpSocket::bind(addr).await.map(UdpSocket::from)
    }

    pub(crate) async fn send_to(
        &mut self,
        buf: &[u8],
        addr: SocketAddr,
    ) -> io::Result<usize> {
        self.inner.send_to(buf, addr).await
    }

    pub(crate) async fn recv_from(
        &mut self,
        buf: &mut [u8],
    ) -> io::Result<(usize, SocketAddr)> {
        self.inner.recv_from(buf).await
    }

    pub(crate) fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}

impl From<net::UdpSocket> for UdpSocket {
    fn from(socket: net::UdpSocket) -> UdpSocket {
        UdpSocket {
            inner: socket,
        }
    }
}

/// Wrap of JoinHandle
pub(crate) struct JoinHandle<T> {
    inner: task::JoinHandle<T>,
}

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        #[cfg(feature = "use-async-std")]
        {
            Pin::new(&mut self.inner).poll(cx)
        }

        #[cfg(feature = "use-tokio")]
        {
            Pin::new(&mut self.inner)
                .poll(cx)
                .map(|joined| joined.expect("tokio task failed to join"))
        }
    }
}

impl<T> From<task::JoinHandle<T>> for JoinHandle<T> {
    fn from(handle: task::JoinHandle<T>) -> JoinHandle<T> {
        JoinHandle {
            inner: handle,
        }
    }
}

pub(crate) fn spawn<F, T>(future: F) -> JoinHandle<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    #[cfg(feature = "use-async-std")]
    {
        async_std::task::spawn(future).into()
    }

    #[cfg(feature = "use-tokio")]
    {
        tokio::spawn(future).into()
    }
}

#[cfg(test)]
pub(crate) fn block_on<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T> + Send,
    T: Send,
{
    #[cfg(feature = "use-async-std")]
    {
        task::block_on(future)
    }

    #[cfg(feature = "use-tokio")]
    {
        let mut rt = tokio::runtime::Runtime::new()
            .expect("failed to start tokio runtime");
        rt.block_on(future)
    }
}

#[cfg(test)]
pub(crate) fn spawn_blocking<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    #[cfg(feature = "use-async-std")]
    {
        task::spawn_blocking(f).into()
    }

    #[cfg(feature = "use-tokio")]
    {
        task::spawn_blocking(f).into()
    }
}

pub(crate) async fn io_timeout<F, T>(dur: Duration, f: F) -> io::Result<T>
where
    F: Future<Output = io::Result<T>>,
{
    #[cfg(feature = "use-async-std")]
    {
        async_std::io::timeout(dur, f).await
    }

    #[cfg(feature = "use-tokio")]
    {
        match tokio::time::timeout(dur, f).await {
            Ok(res) => res,
            Err(tokio::time::Elapsed {
                ..
            }) => Err(io::ErrorKind::TimedOut.into()),
        }
    }
}
