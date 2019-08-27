use futures::io::{AsyncRead, AsyncReadExt};
use futures::{select, FutureExt};
use futures_timer::Delay;
use log::trace;
use std::net::SocketAddr;
use std::time::Duration;

use crate::error::*;
use crate::packet::*;
use crate::wrappers::{udp_socket_bind, UdpSocket};

const DEFAULT_TIMEOUT_SECS: u64 = 3;
const DEFAULT_BLOCK_SIZE: usize = 512;

pub struct ReadRequest<R>
where
    R: AsyncRead + Send,
{
    peer: SocketAddr,
    socket: UdpSocket,
    reader: R,
    block_id: u16,
    block_size: usize,
    timeout: u64,
    oack_opts: Option<Opts>,
}

impl<R> ReadRequest<R>
where
    R: AsyncRead + Send + Unpin,
{
    pub async fn init(
        reader: R,
        file_size: Option<u64>,
        peer: SocketAddr,
        req: RwReq,
    ) -> Result<Self> {
        let mut oack_opts = Opts::default();
        let mut send_oack = false;

        let block_size = match req.opts.block_size {
            Some(size) if size <= 1024 => {
                send_oack = true;
                usize::from(size)
            }
            _ => DEFAULT_BLOCK_SIZE,
        };

        let timeout = match req.opts.timeout {
            Some(timeout) => {
                send_oack = true;
                u64::from(timeout)
            }
            None => DEFAULT_TIMEOUT_SECS,
        };

        if let (Some(0), Some(file_size)) = (req.opts.transfer_size, file_size)
        {
            oack_opts.transfer_size = Some(file_size);
            send_oack = true;
        }

        let oack_opts = if send_oack {
            Some(oack_opts)
        } else {
            None
        };

        Ok(ReadRequest {
            peer,
            socket: udp_socket_bind("0.0.0.0:0").await.map_err(Error::Bind)?,
            reader,
            block_id: 0,
            block_size,
            timeout,
            oack_opts,
        })
    }

    pub async fn handle(&mut self) {
        if let Err(e) = self.try_handle().await {
            let packet = Packet::Error(e.into()).to_bytes();
            // Errors are never retransmitted.
            // We do not care if `send_to` resulted to an IO error.
            let _ = self.socket.send_to(&packet[..], self.peer).await;
        }
    }

    async fn try_handle(&mut self) -> Result<()> {
        // Reply with OACK if needed
        if let Some(opts) = &self.oack_opts {
            trace!("RRQ (peer: {}) - Send OACK: {:?}", self.peer, opts);
            let packet = Packet::OAck(opts.to_owned()).to_bytes();
            self.send(&packet[..]).await?;
        }

        // Send file to client
        loop {
            let block = self.read_block(self.block_size).await?;

            self.block_id = self.block_id.wrapping_add(1);
            let packet = Packet::Data(self.block_id, &block[..]).to_bytes();
            self.send(&packet[..]).await?;

            // exit loop on last block
            if block.len() < self.block_size {
                break;
            }
        }

        trace!("RRQ (peer: {}) - Request served successfully", self.peer);
        Ok(())
    }

    async fn send(&mut self, packet: &[u8]) -> Result<()> {
        let peer = self.peer;
        let block_id = self.block_id;
        let timeout = self.timeout;

        loop {
            self.socket.send_to(&packet[..], self.peer).await?;

            let mut recv_ack_fut = self.recv_ack().boxed().fuse();
            let mut timeout_fut =
                Delay::new(Duration::from_secs(timeout)).fuse();

            select! {
                _ = recv_ack_fut => {
                    trace!("RRQ (peer: {}, block_id: {}) - Received ACK", peer, block_id);
                    break;
                }
                _ = timeout_fut => {
                    trace!("RRQ (peer: {}, block_id: {}) - Timeout", peer, block_id);
                    continue;
                }
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
