use async_io::Async;
use async_lock::Mutex;
use log::trace;
use std::collections::HashSet;
use std::future::Future;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

use super::read_req::*;
use super::write_req::*;
use super::Handler;
use crate::error::*;
use crate::executor::Executor;
use crate::packet::{Packet, RwReq};

/// TFTP server.
pub struct TftpServer<H>
where
    H: Handler,
{
    pub(crate) socket: Async<UdpSocket>,
    pub(crate) handler: Arc<Mutex<H>>,
    pub(crate) reqs_in_progress: Arc<Mutex<HashSet<SocketAddr>>>,
    pub(crate) spawner: Option<crate::executor::Spawner>,
    pub(crate) config: ServerConfig,
}

#[derive(Clone)]
pub(crate) struct ServerConfig {
    pub(crate) timeout: Duration,
    pub(crate) block_size_limit: Option<u16>,
    pub(crate) max_send_retries: u32,
    pub(crate) ignore_client_timeout: bool,
    pub(crate) ignore_client_block_size: bool,
}

pub(crate) const DEFAULT_BLOCK_SIZE: usize = 512;

impl<H: 'static> TftpServer<H>
where
    H: Handler,
{
    /// Returns the listenning socket address.
    pub fn listen_addr(&self) -> Result<SocketAddr> {
        Ok(self.socket.get_ref().local_addr()?)
    }

    /// Consume and start the server.
    pub async fn serve(mut self) -> Result<()> {
        let mut ex = Executor::new();

        self.spawner = Some(ex.spawner());

        ex.run(async move {
            let mut buf = [0u8; 4096];

            loop {
                let (len, peer) = self.socket.recv_from(&mut buf).await?;
                self.handle_req_packet(peer, &buf[..len]).await;
            }
        })
        .await
    }

    async fn handle_req_packet(&self, peer: SocketAddr, data: &[u8]) {
        let packet = match Packet::decode(data) {
            Ok(p @ Packet::Rrq(_)) => p,
            Ok(p @ Packet::Wrq(_)) => p,
            // Ignore packets that are not requests
            Ok(_) => return,
            // Ignore invalid packets
            Err(_) => return,
        };

        if !self.reqs_in_progress.lock().await.insert(peer) {
            // Ignore pending requests
            return;
        }

        match packet {
            Packet::Rrq(req) => self.handle_rrq(peer, req),
            Packet::Wrq(req) => self.handle_wrq(peer, req),
            _ => unreachable!(),
        }
    }

    fn handle_rrq(&self, peer: SocketAddr, req: RwReq) {
        trace!("RRQ recieved (peer: {}, req: {:?})", &peer, &req);

        let handler = Arc::clone(&self.handler);
        let config = self.config.clone();

        // Prepare request future
        let req_fut = async move {
            let (mut reader, size) = handler
                .lock()
                .await
                .read_req_open(&peer, req.filename.as_ref())
                .await
                .map_err(Error::Packet)?;

            let mut read_req =
                ReadRequest::init(&mut reader, size, peer, &req, config)
                    .await?;

            read_req.handle().await;

            Ok(())
        };

        let reqs_in_progress = Arc::clone(&self.reqs_in_progress);

        // Run request future in a new task
        self.spawner
            .as_ref()
            .expect("async_tftp::Server not initialized correctly")
            .spawn(run_req(req_fut, peer, reqs_in_progress));
    }

    fn handle_wrq(&self, peer: SocketAddr, req: RwReq) {
        trace!("WRQ recieved (peer: {}, req: {:?})", &peer, &req);

        let handler = Arc::clone(&self.handler);
        let config = self.config.clone();

        // Prepare request future
        let req_fut = async move {
            let mut writer = handler
                .lock()
                .await
                .write_req_open(
                    &peer,
                    req.filename.as_ref(),
                    req.opts.transfer_size,
                )
                .await
                .map_err(Error::Packet)?;

            let mut write_req =
                WriteRequest::init(&mut writer, peer, &req, config).await?;

            write_req.handle().await;

            Ok(())
        };

        let reqs_in_progress = Arc::clone(&self.reqs_in_progress);

        // Run request future in a new task
        self.spawner
            .as_ref()
            .expect("async_tftp::Server not initialized correctly")
            .spawn(run_req(req_fut, peer, reqs_in_progress));
    }
}

async fn send_error(error: Error, peer: SocketAddr) -> Result<()> {
    let addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
    let socket = Async::<UdpSocket>::bind(addr).map_err(Error::Bind)?;

    let data = Packet::Error(error.into()).to_bytes();
    socket.send_to(&data[..], peer).await?;

    Ok(())
}

async fn run_req(
    req_fut: impl Future<Output = Result<()>>,
    peer: SocketAddr,
    reqs_in_progress: Arc<Mutex<HashSet<SocketAddr>>>,
) {
    if let Err(e) = req_fut.await {
        trace!("Request failed (peer: {}, error: {}", &peer, &e);

        if let Err(e) = send_error(e, peer).await {
            trace!("Failed to send error to peer {}: {}", &peer, &e);
        }
    }

    reqs_in_progress.lock().await.remove(&peer);
}
