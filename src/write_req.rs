use futures::io::AsyncWrite;
use runtime::net::UdpSocket;
use std::net::SocketAddr;

use crate::error::*;
use crate::packet::*;

pub struct WriteRequest<W>
where
    W: AsyncWrite + Send,
{
    peer: SocketAddr,
    req: RwReq,
    socket: UdpSocket,
    block_id: u16,
    writer: W,
}

impl<W> WriteRequest<W>
where
    W: AsyncWrite + Send + Unpin,
{
    pub fn init(writer: W, peer: SocketAddr, req: RwReq) -> Result<Self> {
        Ok(WriteRequest {
            peer,
            req,
            socket: UdpSocket::bind("0.0.0.0:0").map_err(Error::Bind)?,
            block_id: 0,
            writer,
        })
    }

    pub async fn handle(&mut self) {
        if let Err(e) = self.try_handle().await {
            let packet = Packet::from(e).to_bytes();
            // Errors are never retransmitted.
            // We do not care if `send_to` resulted to an IO error.
            let _ = self.socket.send_to(&packet[..], self.peer).await;
        }
    }

    async fn try_handle(&mut self) -> Result<()> {
        unimplemented!();
    }
}
