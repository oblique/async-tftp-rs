use futures::compat::Compat01As03;
use futures_locks::Mutex;
use runtime::net::UdpSocket;
use runtime::task;

use futures::future::select_all;
use futures::FutureExt;
use log::trace;
use std::collections::HashSet;
use std::iter;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use crate::error::*;
use crate::handle::*;
use crate::packet::*;
use crate::read_req::*;
use crate::write_req::*;

pub struct AsyncTftpServer<H>
where
    H: Handle,
{
    socket: Option<UdpSocket>,
    handler: Arc<Mutex<H>>,
    reqs_in_progress: HashSet<SocketAddr>,
}

type ReqResult = std::result::Result<SocketAddr, (SocketAddr, Error)>;

/// This contains all results of the futures that are passed in `select_all`.
enum FutResults {
    /// Result of `recv_req` function.
    RecvReq(Result<(usize, SocketAddr)>, Vec<u8>, UdpSocket),
    /// Result of `req_finished` function.
    ReqFinished(ReqResult),
}

impl<H: 'static> AsyncTftpServer<H>
where
    H: Handle,
{
    pub async fn bind<A>(handler: H, addr: A) -> Result<Self>
    where
        A: ToSocketAddrs,
    {
        Ok(AsyncTftpServer {
            socket: Some(UdpSocket::bind(addr)?),
            handler: Arc::new(Mutex::new(handler)),
            reqs_in_progress: HashSet::new(),
        })
    }

    pub fn with_socket(handler: H, socket: UdpSocket) -> Result<Self> {
        Ok(AsyncTftpServer {
            socket: Some(socket),
            handler: Arc::new(Mutex::new(handler)),
            reqs_in_progress: HashSet::new(),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        let socket =
            self.socket.as_ref().expect("tftp not initialized correctly");
        Ok(socket.local_addr()?)
    }

    pub async fn serve(mut self) -> Result<()> {
        let buf = vec![0u8; 4096];
        let socket =
            self.socket.take().expect("tftp not initialized correctly");

        // Await for the first request
        let recv_req_fut = recv_req(socket, buf).boxed();
        let mut select_fut = select_all(iter::once(recv_req_fut));

        loop {
            let (res, _index, mut remaining_futs) = select_fut.await;

            match res {
                FutResults::RecvReq(res, buf, socket) => {
                    let (len, peer) = res?;

                    if let Some(handle) =
                        self.handle_req_packet(peer, &buf[..len]).await
                    {
                        // Put a future for finished request in the awaiting list
                        let fin_fut = req_finished(handle).boxed();
                        remaining_futs.push(fin_fut);
                    }

                    // Await for another request
                    let recv_req_fut = recv_req(socket, buf).boxed();
                    remaining_futs.push(recv_req_fut);
                }
                // Request finished with an error
                FutResults::ReqFinished(Err((peer, e))) => {
                    trace!(
                        "Failed to handle request for peer {}. Error: {}",
                        peer,
                        e
                    );

                    // Send the error and ignore errors while sending it.
                    let _ = send_error(e, peer).await;
                    self.reqs_in_progress.remove(&peer);
                }
                // Request is served
                FutResults::ReqFinished(Ok(peer)) => {
                    trace!("Request for peer {} is served", peer);
                    self.reqs_in_progress.remove(&peer);
                }
            }

            select_fut = select_all(remaining_futs.into_iter());
        }
    }

    async fn handle_req_packet<'a>(
        &'a mut self,
        peer: SocketAddr,
        data: &'a [u8],
    ) -> Option<task::JoinHandle<ReqResult>> {
        let packet = match Packet::from_bytes(data) {
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

    fn handle_rrq(
        &mut self,
        peer: SocketAddr,
        req: RwReq,
    ) -> task::JoinHandle<ReqResult> {
        let task_handler = Arc::clone(&self.handler);

        task::spawn(async move {
            trace!("RRQ (peer: {}) - {:?}", peer, req);

            let (reader, size) = {
                let mut handler =
                    Compat01As03::new(task_handler.lock()).await.unwrap();

                handler
                    .read_open(&req.filename)
                    .await
                    .map_err(|e| (peer, e.into()))?
            };

            let mut read_req = ReadRequest::init(reader, size, peer, req)
                .await
                .map_err(|e| (peer, e))?;

            read_req.handle().await;

            Ok(peer)
        })
    }

    fn handle_wrq(
        &mut self,
        peer: SocketAddr,
        req: RwReq,
    ) -> task::JoinHandle<ReqResult> {
        let task_handler = Arc::clone(&self.handler);

        task::spawn(async move {
            trace!("WRQ (peer: {}) - {:?}", peer, req);

            let writer = {
                let mut handler =
                    Compat01As03::new(task_handler.lock()).await.unwrap();

                handler
                    .write_open(&req.filename, req.opts.transfer_size)
                    .await
                    .map_err(|e| (peer, e.into()))?
            };

            let mut write_req = WriteRequest::init(writer, peer, req)
                .await
                .map_err(|e| (peer, e))?;

            write_req.handle().await;

            Ok(peer)
        })
    }
}

async fn recv_req(mut socket: UdpSocket, mut buf: Vec<u8>) -> FutResults {
    let res = socket.recv_from(&mut buf).await.map_err(Into::into);
    FutResults::RecvReq(res, buf, socket)
}

async fn req_finished(handle: task::JoinHandle<ReqResult>) -> FutResults {
    let res = handle.await;
    FutResults::ReqFinished(res)
}

async fn send_error(error: Error, peer: SocketAddr) -> Result<()> {
    let mut socket = UdpSocket::bind("0.0.0.0:0").map_err(Error::Bind)?;

    let packet = Packet::Error(error.into()).to_bytes();
    socket.send_to(&packet[..], peer).await?;

    Ok(())
}
