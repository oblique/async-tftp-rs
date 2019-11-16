use async_std::net::UdpSocket;
use bytes::{BufMut, Bytes, BytesMut};
use futures::io::{AsyncRead, AsyncReadExt};
use std::cmp;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{debug_span, trace};
use tracing_futures::Instrument;

use crate::error::*;
use crate::packet::*;
use crate::server::{ServerConfig, DEFAULT_BLOCK_SIZE};

pub(crate) struct ReadRequest<'r, R>
where
    R: AsyncRead + Send,
{
    peer: SocketAddr,
    socket: UdpSocket,
    reader: &'r mut R,
    buffer: BytesMut,
    block_id: u16,
    block_size: usize,
    timeout: Duration,
    oack_opts: Option<Opts>,
}

impl<'r, R> ReadRequest<'r, R>
where
    R: AsyncRead + Send + Unpin,
{
    pub(crate) async fn init(
        reader: &'r mut R,
        file_size: Option<u64>,
        peer: SocketAddr,
        req: &RwReq,
        config: ServerConfig,
    ) -> Result<ReadRequest<'r, R>> {
        let oack_opts = build_oack_opts(&config, req, file_size);

        let block_size = oack_opts
            .as_ref()
            .and_then(|o| o.block_size)
            .map(usize::from)
            .unwrap_or(DEFAULT_BLOCK_SIZE);

        let timeout = oack_opts
            .as_ref()
            .and_then(|o| o.timeout)
            .map(|t| Duration::from_secs(u64::from(t)))
            .unwrap_or(config.timeout);

        Ok(ReadRequest {
            peer,
            socket: UdpSocket::bind("0.0.0.0:0").await.map_err(Error::Bind)?,
            reader,
            buffer: BytesMut::with_capacity(
                PACKET_DATA_HEADER_LEN + block_size,
            ),
            block_id: 0,
            block_size,
            timeout,
            oack_opts,
        })
    }

    pub(crate) async fn handle(&mut self) {
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

            self.send(buf, 0).await?;
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
            self.send(buf, self.block_id).instrument(span).await?;

            if is_last_block {
                break;
            }
        }

        trace!("Request served successfully");
        Ok(())
    }

    async fn send(&mut self, packet: Bytes, block_id: u16) -> Result<()> {
        // Send packet until we receive an ack
        loop {
            self.socket.send_to(&packet[..], self.peer).await?;

            match self.recv_ack().await {
                Ok(_) => {
                    // TODO: Remove ugly logs and provide interface to the user
                    log::debug!("RRQ (block_id: {}) - Received ACK", block_id);
                    trace!("Received ACK");
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                    log::debug!("RRQ (block_id: {}) - Timeout", block_id);
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

fn build_oack_opts(
    config: &ServerConfig,
    req: &RwReq,
    file_size: Option<u64>,
) -> Option<Opts> {
    let mut opts = Opts::default();

    if !config.ignore_client_block_size {
        opts.block_size = match (req.opts.block_size, config.maximum_block_size)
        {
            (Some(bsize), Some(max_bsize)) => Some(cmp::min(bsize, max_bsize)),
            (Some(bsize), None) => Some(bsize),
            _ => None,
        };
    }

    if !config.ignore_client_timeout {
        opts.timeout = req.opts.timeout;
    }

    if let (Some(0), Some(file_size)) = (req.opts.transfer_size, file_size) {
        opts.transfer_size = Some(file_size);
    }

    if opts == Opts::default() {
        None
    } else {
        Some(opts)
    }
}
