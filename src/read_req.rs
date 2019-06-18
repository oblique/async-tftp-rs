use futures::io::{AsyncRead, AsyncReadExt};
use futures::{select, FutureExt};
use runtime::net::UdpSocket;
use std::net::SocketAddr;
use std::time::Duration;

use crate::error::*;
use crate::packet::*;
use crate::utils::delay;

const DEFAULT_TIMEOUT_SECS: u8 = 3;
const DEFAULT_BLOCK_SIZE: u16 = 512;

pub struct ReadRequest<R>
where
    R: AsyncRead + Send,
{
    peer: SocketAddr,
    req: RwReq,
    socket: UdpSocket,
    block_id: u16,
    reader: R,
    size: Option<u64>,
}

impl<R> ReadRequest<R>
where
    R: AsyncRead + Send + Unpin,
{
    pub fn init(
        reader: R,
        size: Option<u64>,
        peer: SocketAddr,
        req: RwReq,
    ) -> Result<Self> {
        Ok(ReadRequest {
            peer,
            req,
            socket: UdpSocket::bind("0.0.0.0:0").map_err(Error::Bind)?,
            block_id: 0,
            reader,
            size,
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
        let mut oack_opts = self.req.opts.clone();

        // Server needs to reply the size of the actual file
        oack_opts.transfer_size = match self.req.opts.transfer_size {
            Some(0) => self.size,
            _ => None,
        };

        // Reply with OACK if needed
        if !oack_opts.all_none() {
            let packet = Packet::OAck(oack_opts).to_bytes();
            self.send(&packet[..]).await?;
        }

        let block_size =
            self.req.opts.block_size.unwrap_or(DEFAULT_BLOCK_SIZE).into();

        // Send file to client
        loop {
            let block = self.read_block(block_size).await?;
            let last_block = block.len() < block_size;

            self.block_id = self.block_id.wrapping_add(1);
            let packet = Packet::Data(self.block_id, block).to_bytes();
            self.send(&packet[..]).await?;

            if last_block {
                break;
            }
        }

        Ok(())
    }

    async fn send<'a>(&'a mut self, packet: &'a [u8]) -> Result<()> {
        let timeout =
            self.req.opts.timeout.unwrap_or(DEFAULT_TIMEOUT_SECS).into();

        loop {
            self.socket.send_to(&packet[..], self.peer).await?;

            let mut recv_ack_fut = self.recv_ack().boxed().fuse();
            let mut timeout_fut = delay(Duration::from_secs(timeout));

            select! {
                _ = recv_ack_fut => break,
                _ = timeout_fut => continue,
            };
        }

        Ok(())
    }

    async fn recv_ack(&mut self) -> Result<()> {
        let mut buf = [0u8; 1024];

        loop {
            let (len, peer) = self.socket.recv_from(&mut buf[..]).await?;

            // if the packet do not come from the client we are serving, then ignore it
            if peer != self.peer {
                continue;
            }

            // parse only valid Ack packets, the rest are ignored
            if let Ok(Packet::Ack(block_id)) = Packet::from_bytes(&buf[..len]) {
                if self.block_id == block_id {
                    break;
                }
            }
        }

        Ok(())
    }

    async fn read_block(&mut self, block_size: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; block_size];
        let mut len = 0;

        while len < block_size {
            match self.reader.read(&mut buf[len..]).await? {
                0 => break,
                x => len += x,
            }
        }

        buf.truncate(len);
        Ok(buf)
    }
}
