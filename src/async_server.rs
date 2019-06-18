use futures::future::select_all;
use futures::FutureExt;
use log::error;
use runtime::net::UdpSocket;
use runtime::task::JoinHandle;
use std::collections::HashSet;
use std::iter;
use std::net::{SocketAddr, ToSocketAddrs};

use crate::error::*;
use crate::handle::*;
use crate::packet::*;
use crate::read_req::*;

pub struct AsyncTftpServer<H>
where
    H: ReadHandle,
{
    socket: Option<UdpSocket>,
    read_handle: H,
    reqs_in_progress: HashSet<SocketAddr>,
}

enum FutResults {
    RecvFrom(Result<(usize, SocketAddr)>, Vec<u8>, UdpSocket),
    ReqFinished(SocketAddr),
}

impl<H> AsyncTftpServer<H>
where
    H: ReadHandle,
{
    pub fn bind<A>(read_handle: H, addr: A) -> Result<Self>
    where
        A: ToSocketAddrs,
    {
        Ok(AsyncTftpServer {
            socket: Some(UdpSocket::bind(addr)?),
            read_handle,
            reqs_in_progress: HashSet::new(),
        })
    }

    pub fn with_socket(socket: UdpSocket, read_handle: H) -> Result<Self> {
        Ok(AsyncTftpServer {
            socket: Some(socket),
            read_handle,
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

        let recv_fut = recv_from(socket, buf).boxed();
        let mut select_fut = select_all(iter::once(recv_fut));

        loop {
            let (res, _index, mut remaining) = select_fut.await;

            match res {
                FutResults::RecvFrom(res, buf, socket) => {
                    let (len, peer) = res?;

                    match self.handle_req_packet(peer, &buf[..len]) {
                        Ok(Some(handle)) => {
                            let fin_fut = req_finished(handle).boxed();
                            remaining.push(fin_fut);
                        }
                        // Request already in progress
                        Ok(None) => {}
                        Err(e) => send_error(e, peer).await?,
                    }

                    let recv_fut = recv_from(socket, buf).boxed();
                    remaining.push(recv_fut);
                }
                FutResults::ReqFinished(peer) => {
                    self.reqs_in_progress.remove(&peer);
                }
            }

            select_fut = select_all(remaining.into_iter());
        }
    }

    fn handle_req_packet(
        &mut self,
        peer: SocketAddr,
        data: &[u8],
    ) -> Result<Option<JoinHandle<SocketAddr>>> {
        let packet = match Packet::from_bytes(data) {
            Ok(x) => x,
            // Ignore invalid packets
            Err(_) => return Ok(None),
        };

        let res = match packet {
            Packet::Rrq(req) => self.handle_rrq(peer, req),
            Packet::Wrq(_req) => {
                unimplemented!();
            }
            // Ignore packets that are not requests
            _ => return Ok(None),
        };

        if let Err(ref e) = res {
            if let Error::Bind(e) = e {
                error!("tftp: Failed to bind socket: {}", e);
                return Ok(None);
            }
        }

        res
    }

    fn handle_rrq(
        &mut self,
        peer: SocketAddr,
        req: RwReq,
    ) -> Result<Option<JoinHandle<SocketAddr>>> {
        if self.reqs_in_progress.insert(peer) {
            let (reader, size) = self.read_handle.open(&req.filename)?;
            let mut read_req = ReadRequest::init(reader, size, peer, req)?;

            let handle = runtime::spawn(async move {
                read_req.handle().await;
                peer
            });

            Ok(Some(handle))
        } else {
            Ok(None)
        }
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
