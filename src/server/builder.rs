use async_executor::Executor;
use async_io::Async;
use async_mutex::Mutex;
use std::collections::HashSet;
use std::net::{SocketAddr, UdpSocket};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use super::handlers::{DirHandler, DirHandlerMode};
use super::{Handler, ServerConfig, TftpServer};
use crate::error::{Error, Result};

/// TFTP server builder.
pub struct TftpServerBuilder<H: Handler> {
    handle: H,
    addr: SocketAddr,
    socket: Option<Async<UdpSocket>>,
    timeout: Duration,
    block_size_limit: Option<u16>,
    max_send_retries: u32,
    ignore_client_timeout: bool,
    ignore_client_block_size: bool,
}

impl TftpServerBuilder<DirHandler> {
    /// Create new buidler with [`DirHandler`] that serves only read requests.
    ///
    /// [`DirHandler`]: handlers/struct.DirHandler.html
    pub fn with_dir_ro<P>(dir: P) -> Result<TftpServerBuilder<DirHandler>>
    where
        P: AsRef<Path>,
    {
        let handler = DirHandler::new(dir, DirHandlerMode::ReadOnly)?;
        Ok(TftpServerBuilder::with_handler(handler))
    }

    /// Create new buidler with [`DirHandler`] that serves only write requests.
    pub fn with_dir_wo<P>(dir: P) -> Result<TftpServerBuilder<DirHandler>>
    where
        P: AsRef<Path>,
    {
        let handler = DirHandler::new(dir, DirHandlerMode::WriteOnly)?;
        Ok(TftpServerBuilder::with_handler(handler))
    }

    /// Create new buidler with [`DirHandler`] that serves read and write requests.
    pub fn with_dir_rw<P>(dir: P) -> Result<TftpServerBuilder<DirHandler>>
    where
        P: AsRef<Path>,
    {
        let handler = DirHandler::new(dir, DirHandlerMode::ReadWrite)?;
        Ok(TftpServerBuilder::with_handler(handler))
    }
}

impl<H: Handler> TftpServerBuilder<H> {
    /// Create new builder with custom [`Handler`].
    pub fn with_handler(handler: H) -> Self {
        TftpServerBuilder {
            handle: handler,
            addr: "0.0.0.0:69".parse().unwrap(),
            socket: None,
            timeout: Duration::from_secs(3),
            block_size_limit: None,
            max_send_retries: 100,
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
    pub fn socket(self, socket: Async<UdpSocket>) -> Self {
        TftpServerBuilder {
            socket: Some(socket),
            ..self
        }
    }

    /// Set underling UDP socket.
    pub fn std_socket(self, socket: UdpSocket) -> Result<Self> {
        let socket = Async::new(socket)?;

        Ok(TftpServerBuilder {
            socket: Some(socket),
            ..self
        })
    }

    /// Set retry timeout.
    ///
    /// Client can override this (RFC2349). If you want to enforce it you must
    /// combine it [`ignore_client_timeout`](Self::ignore_client_timeout).
    ///
    /// This crate allows you to set non-standard timeouts (i.e. timeouts that are less
    /// than a second). However if you choose to do it make sure you test it well in your
    /// environment since client's behavior is undefined.
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
    pub fn block_size_limit(self, size: u16) -> Self {
        TftpServerBuilder {
            block_size_limit: Some(size),
            ..self
        }
    }

    /// Set maximum send retries for a data block.
    ///
    /// On timeout server will try to send the data block again. When retries are
    /// reached for the specific data block the server closes the connection with
    /// the client.
    ///
    /// Default: 100 retries.
    pub fn max_send_retries(self, retries: u32) -> Self {
        TftpServerBuilder {
            max_send_retries: retries,
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
    /// This will enforce 512 block size that is defined in RFC1350.
    pub fn ignore_client_block_size(self) -> Self {
        TftpServerBuilder {
            ignore_client_block_size: true,
            ..self
        }
    }

    /// Build [`TftpServer`].
    pub async fn build(mut self) -> Result<TftpServer<H>> {
        let socket = match self.socket.take() {
            Some(socket) => socket,
            None => Async::<UdpSocket>::bind(self.addr).map_err(Error::Bind)?,
        };

        let config = ServerConfig {
            timeout: self.timeout,
            block_size_limit: self.block_size_limit,
            max_send_retries: self.max_send_retries,
            ignore_client_timeout: self.ignore_client_timeout,
            ignore_client_block_size: self.ignore_client_block_size,
        };

        Ok(TftpServer {
            socket,
            handler: Arc::new(Mutex::new(self.handle)),
            reqs_in_progress: Arc::new(Mutex::new(HashSet::new())),
            ex: Executor::new(),
            config,
        })
    }
}
