use std::io;
use std::net::{SocketAddr, ToSocketAddrs};

#[cfg(not(feature = "asyncstd"))]
mod inner {
    use futures::compat::Compat01As03;
    pub use futures_locks::{Mutex, MutexGuard};
    pub use runtime::net::UdpSocket;
    pub use runtime::task;
    use std::io;
    use std::net::ToSocketAddrs;

    pub async fn udp_socket_bind<A: ToSocketAddrs>(
        addr: A,
    ) -> io::Result<UdpSocket> {
        UdpSocket::bind(addr)
    }

    pub async fn mutex_lock<T>(mutex: &Mutex<T>) -> MutexGuard<T> {
        Compat01As03::new(mutex.lock()).await.unwrap()
    }
}

#[cfg(feature = "asyncstd")]
mod inner {
    pub use async_std::net::UdpSocket;
    pub use async_std::sync::{Mutex, MutexGuard};
    pub use async_std::task;
    use std::io;
    use std::net::ToSocketAddrs;

    pub async fn udp_socket_bind<A: ToSocketAddrs>(
        addr: A,
    ) -> io::Result<UdpSocket> {
        UdpSocket::bind(addr).await
    }

    pub async fn mutex_lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex.lock().await
    }
}

pub use self::inner::*;

// `runtime` and `async-std` have different mutability
pub async fn socket_recv_from(
    socket: &mut UdpSocket,
    buf: &mut [u8],
) -> io::Result<(usize, SocketAddr)> {
    socket.recv_from(buf).await
}

// `runtime` and `async-std` have different mutability
pub async fn socket_send_to<A: ToSocketAddrs>(
    socket: &mut UdpSocket,
    buf: &[u8],
    addrs: A,
) -> io::Result<usize> {
    socket.send_to(buf, addrs).await
}
