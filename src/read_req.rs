use async_std::net::UdpSocket;
use bytes::{BufMut, Bytes, BytesMut};
use futures::io::{AsyncRead, AsyncReadExt};
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{debug_span, trace};
use tracing_futures::Instrument;

use crate::error::*;
use crate::packet::*;

const DEFAULT_TIMEOUT_SECS: Duration = Duration::from_secs(3);
const DEFAULT_BLOCK_SIZE: usize = 512;

pub struct ReadRequest<'r, R>
where
    R: AsyncRead + Send,
{
    peer: SocketAddr,
    socket: UdpSocket,
    reader: &'r mut R,
    block_id: u16,
    block_size: usize,
    timeout: Duration,
    oack_opts: Option<Opts>,
    buffer: BytesMut,
}

impl<'r, R> ReadRequest<'r, R>
where
    R: AsyncRead + Send + Unpin,
{
    pub async fn init(
        reader: &'r mut R,
        file_size: Option<u64>,
        peer: SocketAddr,
        req: &RwReq,
    ) -> Result<ReadRequest<'r, R>> {
        let mut oack_opts = Opts::default();
        let mut send_oack = false;

        let block_size = match req.opts.block_size {
            Some(size) if size <= 1024 => {
                send_oack = true;
                oack_opts.block_size = Some(size);
                usize::from(size)
            }
            _ => DEFAULT_BLOCK_SIZE,
        };

        let timeout = match req.opts.timeout {
            Some(timeout) => {
                send_oack = true;
                oack_opts.timeout = Some(timeout);
                Duration::from_secs(u64::from(timeout))
            }
            None => DEFAULT_TIMEOUT_SECS,
        };

        if let (Some(0), Some(file_size)) = (req.opts.transfer_size, file_size)
        {
            send_oack = true;
            oack_opts.transfer_size = Some(file_size);
        }

        let oack_opts = if send_oack {
            Some(oack_opts)
        } else {
            None
        };

        Ok(ReadRequest {
            peer,
            socket: UdpSocket::bind("0.0.0.0:0").await.map_err(Error::Bind)?,
            reader,
            block_id: 0,
            block_size,
            timeout,
            oack_opts,
            buffer: BytesMut::with_capacity(
                PACKET_DATA_HEADER_LEN + block_size,
            ),
        })
    }

    pub async fn handle(&mut self) {
        if let Err(e) = self.try_handle().await {
            Packet::Error(e.into()).encode(&mut self.buffer);
            let buf = self.buffer.take().freeze();
            // Errors are never retransmitted.
            // We do not care if `send_to` resulted to an IO error.
            let _ = self.socket.send_to(&buf[..], self.peer).await;
        }
    }

    async fn try_handle(&mut self) -> Result<()> {
        // Reply with OACK if needed
        if let Some(opts) = &self.oack_opts {
            trace!("Send OACK: {:?}", opts);

            Packet::OAck(opts.to_owned()).encode(&mut self.buffer);
            let buf = self.buffer.take().freeze();

            self.send(buf).await?;
        }

        // Send file to client
        loop {
            let is_last_block;

            // Reclaim buffer
            self.buffer.reserve(PACKET_DATA_HEADER_LEN + self.block_size);

            // Encode head of Data packet
            self.block_id = self.block_id.wrapping_add(1);
            Packet::encode_data_head(self.block_id, &mut self.buffer);

            // Read block in self.buffer
            let buf = unsafe {
                let mut buf =
                    self.buffer.split_to(self.buffer.len() + self.block_size);

                let len = self.read_block(buf.bytes_mut()).await?;
                is_last_block = len < self.block_size;

                buf.advance_mut(len);
                buf.freeze()
            };

            // Send Data packet
            let span = debug_span!("", block_id = %self.block_id);
            self.send(buf).instrument(span).await?;

            if is_last_block {
                break;
            }
        }

        trace!("Request served successfully");
        Ok(())
    }

    async fn send(&mut self, packet: Bytes) -> Result<()> {
        // Send packet until we receive an ack
        loop {
            self.socket.send_to(&packet[..], self.peer).await?;

            match self.recv_ack().await {
                Ok(_) => {
                    trace!("Received ACK");
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                    trace!("Timeout");
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(())
    }

    async fn recv_ack(&mut self) -> io::Result<()> {
        // We can not use `self` within `async_std::io::timeout` because not all
        // struct members implement `Sync`. So we borrow only what we need.
        let socket = &mut self.socket;
        let peer = self.peer;
        let block_id = self.block_id;

        async_std::io::timeout(self.timeout, async {
            let mut buf = [0u8; 1024];

            loop {
                let (len, recved_peer) = socket.recv_from(&mut buf[..]).await?;

                // if the packet do not come from the client we are serving, then ignore it
                if recved_peer != peer {
                    continue;
                }

                // parse only valid Ack packets, the rest are ignored
                if let Ok(Packet::Ack(recved_block_id)) =
                    Packet::decode(&buf[..len])
                {
                    if recved_block_id == block_id {
                        return Ok(());
                    }
                }
            }
        })
        .await?;

        Ok(())
    }

    async fn read_block(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut len = 0;

        while len < buf.len() {
            match self.reader.read(&mut buf[len..]).await? {
                0 => break,
                x => len += x,
            }
        }

        Ok(len)
    }
}
