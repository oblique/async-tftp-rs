use async_std::net::UdpSocket;
use async_std::sync::Mutex;
use bytes::BytesMut;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use super::handlers::DirRoHandler;
use super::{Handler, ServerConfig, TftpServer};
use crate::error::Result;

pub struct TftpServerBuilder<H: Handler> {
    handle: H,
    addr: SocketAddr,
    socket: Option<UdpSocket>,
    timeout: Duration,
    maximum_block_size: Option<u16>,
    ignore_client_timeout: bool,
    ignore_client_block_size: bool,
}

impl TftpServerBuilder<DirRoHandler> {
    /// Create new buidler that handles only read requests for a directory.
    pub fn with_dir_ro<P>(dir: P) -> Result<TftpServerBuilder<DirRoHandler>>
    where
        P: AsRef<Path>,
    {
        let handler = DirRoHandler::new(dir)?;
        Ok(TftpServerBuilder::with_handler(handler))
    }
}

impl<H: Handler> TftpServerBuilder<H> {
    /// Create new builder.
    pub fn with_handler(handler: H) -> Self {
        TftpServerBuilder {
            handle: handler,
            addr: "0.0.0.0:69".parse().unwrap(),
            socket: None,
            timeout: Duration::from_secs(3),
            maximum_block_size: None,
            ignore_client_timeout: false,
            ignore_client_block_size: false,
        }
    }

    /// Set listening address.
    ///
    /// This is ignored if underling socket is set.
    ///
    /// **Default:** `0.0.0.0:69`
    pub fn bind(self, addr: SocketAddr) -> Self {
        TftpServerBuilder {
            addr,
            ..self
        }
    }

    /// Set underling UDP socket.
    pub fn socket(self, socket: UdpSocket) -> Self {
        TftpServerBuilder {
            socket: Some(socket),
            ..self
        }
    }

    /// Set retry timeout.
    ///
    /// You may need to combine this with `ignore_client_timeout`.
    ///
    /// Based on In RFC2349 timeout should be between 1-255 seconds, but this
    /// crate supports non-standard timeout, which can be anything, even milliseconds.
    ///
    /// If you choose to use non-standard timeout then make sure you test it well
    /// in your environment since client's behavior is undefined.
    ///
    /// **Default:** 3 seconds
    pub fn timeout(self, timeout: Duration) -> Self {
        TftpServerBuilder {
            timeout,
            ..self
        }
    }

    /// Set maximum block size.
    ///
    /// Client can request a specific block size (RFC2348). Use this option if you
    /// want to set a limit.
    ///
    /// **Real life scenario:** U-Boot does not support IP fragmentation and requests
    /// block size of 1468. This works fine if your MTU is 1500 bytes, however if
    /// you are accessing client through a VPN, then transfer will never start. Use
    /// this option to workaround the problem.
    pub fn maximum_block_size(self, size: u16) -> Self {
        TftpServerBuilder {
            maximum_block_size: Some(size),
            ..self
        }
    }

    /// Ignore client's `timeout` option.
    ///
    /// With this you enforce server's timeout by ignoring client's
    /// `timeout` option of RFC2349.
    pub fn ignore_client_timeout(self) -> Self {
        TftpServerBuilder {
            ignore_client_timeout: true,
            ..self
        }
    }

    /// Ignore client's block size option.
    ///
    /// With this you can ignore client's `blksize` option of RFC2348.
    /// This will have as a result a 512 block size that is defined in RFC1350.
    pub fn ignore_client_block_size(self) -> Self {
        TftpServerBuilder {
            ignore_client_block_size: true,
            ..self
        }
    }

    /// Build `TftpServer`.
    pub async fn build(mut self) -> Result<TftpServer<H>> {
        let socket = match self.socket.take() {
            Some(socket) => socket,
            None => UdpSocket::bind(self.addr).await?,
        };

        let config = ServerConfig {
            timeout: self.timeout,
            maximum_block_size: self.maximum_block_size,
            ignore_client_timeout: self.ignore_client_timeout,
            ignore_client_block_size: self.ignore_client_block_size,
        };

        Ok(TftpServer {
            socket: Some(socket),
            handler: Arc::new(Mutex::new(self.handle)),
            config,
            reqs_in_progress: HashSet::new(),
            buffer: BytesMut::new(),
        })
    }
}
