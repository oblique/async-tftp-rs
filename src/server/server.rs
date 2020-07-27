use async_executor::Task;
use async_lock::Lock;
use async_net::UdpSocket;
use bytes::BytesMut;
use futures_lite::{FutureExt, StreamExt};
use futures_util::stream::FuturesUnordered;
use log::trace;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use super::read_req::*;
use super::write_req::*;
use super::Handler;
use crate::error::*;
use crate::packet::{Packet, RwReq};
use crate::task;

/// TFTP server.
pub struct TftpServer<H>
where
    H: Handler,
{
    pub(crate) socket: Option<UdpSocket>,
    pub(crate) handler: Arc<Lock<H>>,
    pub(crate) config: ServerConfig,
    pub(crate) reqs_in_progress: HashSet<SocketAddr>,
    pub(crate) buffer: BytesMut,
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

type ReqResult = std::result::Result<SocketAddr, (SocketAddr, Error)>;

/// This contains all results of the futures that are passed in `FuturesUnordered`.
enum FutResults {
    /// Result of `recv_req` function.
    RecvReq(Result<(usize, SocketAddr)>, Vec<u8>, UdpSocket),
    /// Result of `req_finished` function.
    ReqFinished(ReqResult),
}

impl<H: 'static> TftpServer<H>
where
    H: Handler,
{
    /// Returns the listenning socket address.
    pub fn listen_addr(&self) -> Result<SocketAddr> {
        let socket =
            self.socket.as_ref().expect("tftp not initialized correctly");
        Ok(socket.local_addr()?)
    }

    /// Consume and start the server.
    pub async fn serve(mut self) -> Result<()> {
        let mut futs = FuturesUnordered::new();
        let buf = vec![0u8; 4096];
        let socket =
            self.socket.take().expect("tftp not initialized correctly");

        // Await for the first request
        let recv_req_fut = recv_req(socket, buf).boxed();
        futs.push(recv_req_fut);

        while let Some(res) = futs.next().await {
            match res {
                FutResults::RecvReq(res, buf, socket) => {
                    let (len, peer) = res?;

                    if let Some(handle) =
                        self.handle_req_packet(peer, &buf[..len]).await
                    {
                        // Put a future for finished request in the awaiting list
                        let fin_fut = req_finished(handle).boxed();
                        futs.push(fin_fut);
                    }

                    // Await for another request
                    let recv_req_fut = recv_req(socket, buf).boxed();
                    futs.push(recv_req_fut);
                }
                // Request finished with an error
                FutResults::ReqFinished(Err((peer, e))) => {
                    trace!("Request failed (peer: {}, error: {}", &peer, &e);

                    // Send the error and ignore errors while sending it.
                    let _ = self.send_error(e, peer).await;
                    self.reqs_in_progress.remove(&peer);
                }
                // Request is served
                FutResults::ReqFinished(Ok(peer)) => {
                    self.reqs_in_progress.remove(&peer);
                }
            }
        }

        Ok(())
    }

    async fn handle_req_packet<'a>(
        &'a mut self,
        peer: SocketAddr,
        data: &'a [u8],
    ) -> Option<Task<ReqResult>> {
        let packet = match Packet::decode(data) {
            Ok(packet) => match packet {
                Packet::Rrq(_) | Packet::Wrq(_) => packet,
                // Ignore packets that are not requests
                _ => return None,
            },
            // Ignore invalid packets
            Err(_) => return None,
        };

        if !self.reqs_in_progress.insert(peer) {
            // Ignore pending requests
            return None;
        }

        match packet {
            Packet::Rrq(req) => Some(self.handle_rrq(peer, req)),
            Packet::Wrq(req) => Some(self.handle_wrq(peer, req)),
            _ => None,
        }
    }

    fn handle_rrq(&mut self, peer: SocketAddr, req: RwReq) -> Task<ReqResult> {
        trace!("RRQ recieved (peer: {}, req: {:?})", &peer, &req);

        let handler = Arc::clone(&self.handler);
        let config = self.config.clone();

        task::spawn(async move {
            let (mut reader, size) = handler
                .lock()
                .await
                .read_req_open(&peer, req.filename.as_ref())
                .await
                .map_err(|e| (peer, Error::Packet(e)))?;

            let mut read_req =
                ReadRequest::init(&mut reader, size, peer, &req, config)
                    .await
                    .map_err(|e| (peer, e))?;

            read_req.handle().await;

            Ok(peer)
        })
    }

    fn handle_wrq(&mut self, peer: SocketAddr, req: RwReq) -> Task<ReqResult> {
        trace!("WRQ recieved (peer: {}, req: {:?})", &peer, &req);

        let handler = Arc::clone(&self.handler);
        let config = self.config.clone();

        task::spawn(async move {
            let mut writer = handler
                .lock()
                .await
                .write_req_open(
                    &peer,
                    req.filename.as_ref(),
                    req.opts.transfer_size,
                )
                .await
                .map_err(|e| (peer, Error::Packet(e)))?;

            let mut write_req =
                WriteRequest::init(&mut writer, peer, &req, config)
                    .await
                    .map_err(|e| (peer, e))?;

            write_req.handle().await;

            Ok(peer)
        })
    }

    async fn send_error(
        &mut self,
        error: Error,
        peer: SocketAddr,
    ) -> Result<()> {
        Packet::Error(error.into()).encode(&mut self.buffer);
        let buf = self.buffer.split().freeze();

        let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(Error::Bind)?;
        socket.send_to(&buf[..], peer).await?;

        Ok(())
    }
}

async fn recv_req(socket: UdpSocket, mut buf: Vec<u8>) -> FutResults {
    let res = socket.recv_from(&mut buf).await.map_err(Into::into);
    FutResults::RecvReq(res, buf, socket)
}

async fn req_finished(handle: Task<ReqResult>) -> FutResults {
    let res = handle.await;
    FutResults::ReqFinished(res)
}
