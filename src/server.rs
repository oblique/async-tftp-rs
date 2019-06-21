use futures::future::select_all;
use futures::FutureExt;
use log::trace;
use runtime::net::UdpSocket;
use runtime::task::JoinHandle;
use std::collections::HashSet;
use std::iter;
use std::net::{SocketAddr, ToSocketAddrs};

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
    handler: H,
    reqs_in_progress: HashSet<SocketAddr>,
}

/// This contains all results of the futures that are passed in `select_all`.
enum FutResults {
    /// Result of `recv_from` function.
    RecvFrom(Result<(usize, SocketAddr)>, Vec<u8>, UdpSocket),
    /// Result of `req_finished` function.
    ReqFinished(SocketAddr),
}

impl<H> AsyncTftpServer<H>
where
    H: Handle,
{
    pub fn bind<A>(handler: H, addr: A) -> Result<Self>
    where
        A: ToSocketAddrs,
    {
        Ok(AsyncTftpServer {
            socket: Some(UdpSocket::bind(addr)?),
            handler,
            reqs_in_progress: HashSet::new(),
        })
    }

    pub fn with_socket(handler: H, socket: UdpSocket) -> Result<Self> {
        Ok(AsyncTftpServer {
            socket: Some(socket),
            handler,
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
        let recv_req_fut = recv_from(socket, buf).boxed();
        let mut select_fut = select_all(iter::once(recv_req_fut));

        loop {
            let (res, _index, mut remaining_futs) = select_fut.await;

            match res {
                FutResults::RecvFrom(res, buf, socket) => {
                    let (len, peer) = res?;

                    if let Some(handle) =
                        self.handle_req_packet(peer, &buf[..len]).await
                    {
                        // Put a future for finished request in the awaiting list
                        let fin_fut = req_finished(handle).boxed();
                        remaining_futs.push(fin_fut);
                    }

                    // Await for another request
                    let recv_req_fut = recv_from(socket, buf).boxed();
                    remaining_futs.push(recv_req_fut);
                }
                FutResults::ReqFinished(peer) => {
                    // Request is served
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
    ) -> Option<JoinHandle<SocketAddr>> {
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

        let res = match packet {
            Packet::Rrq(req) => self.handle_rrq(peer, req),
            Packet::Wrq(req) => self.handle_wrq(peer, req),
            // Any other packet types are already handled above
            _ => unreachable!(),
        };

        match res {
            Ok(handle) => Some(handle),
            Err(e) => {
                trace!(
                    "Failed to handle request for peer {}. Error: {}",
                    peer,
                    e
                );

                // Send the error and ignore errors while sending it.
                let _ = send_error(e, peer).await;

                // Request is served, with an error.
                self.reqs_in_progress.remove(&peer);
                None
            }
        }
    }

    fn handle_rrq(
        &mut self,
        peer: SocketAddr,
        req: RwReq,
    ) -> Result<JoinHandle<SocketAddr>> {
        trace!("RRQ (peer: {}) - {:?}", peer, req);

        let (reader, size) = self.handler.read_open(&req.filename)?;
        let mut read_req = ReadRequest::init(reader, size, peer, req)?;

        let handle = runtime::spawn(async move {
            read_req.handle().await;
            peer
        });

        Ok(handle)
    }

    fn handle_wrq(
        &mut self,
        peer: SocketAddr,
        req: RwReq,
    ) -> Result<JoinHandle<SocketAddr>> {
        trace!("WRQ (peer: {}) - {:?}", peer, req);

        let writer =
            self.handler.write_open(&req.filename, req.opts.transfer_size)?;
        let mut write_req = WriteRequest::init(writer, peer, req)?;

        let handle = runtime::spawn(async move {
            write_req.handle().await;
            peer
        });

        Ok(handle)
    }
}

async fn recv_from(mut socket: UdpSocket, mut buf: Vec<u8>) -> FutResults {
    let res = socket.recv_from(&mut buf).await.map_err(Into::into);
    FutResults::RecvFrom(res, buf, socket)
}

async fn req_finished(handle: JoinHandle<SocketAddr>) -> FutResults {
    let res = handle.await;
    FutResults::ReqFinished(res)
}

async fn send_error(error: Error, peer: SocketAddr) -> Result<()> {
    let mut socket = UdpSocket::bind("0.0.0.0:0").map_err(Error::Bind)?;

    let packet = Packet::from(error).to_bytes();
    socket.send_to(&packet[..], peer).await?;

    Ok(())
}
