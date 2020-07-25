use async_net::UdpSocket;
use bytes::{Buf, Bytes, BytesMut};
use futures_lite::{AsyncWrite, AsyncWriteExt};
use std::cmp;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::packet::{Opts, Packet, RwReq, PACKET_DATA_HEADER_LEN};
use crate::server::{ServerConfig, DEFAULT_BLOCK_SIZE};
use crate::utils::io_timeout;

pub(crate) struct WriteRequest<'w, W>
where
    W: AsyncWrite + Send,
{
    peer: SocketAddr,
    socket: UdpSocket,
    writer: &'w mut W,
    // BytesMut reclaims memory only if it is continuous.
    // Because we always need to keep the previous ACK, we can not use
    // `buffer` as its storage since it breaks the continuity.
    // So we keep previous ACK in `ack` buffer.
    buffer: BytesMut,
    ack: BytesMut,
    block_size: usize,
    timeout: Duration,
    max_retries: u32,
    oack_opts: Option<Opts>,
}

impl<'w, W> WriteRequest<'w, W>
where
    W: AsyncWrite + Send + Unpin,
{
    pub(crate) async fn init(
        writer: &'w mut W,
        peer: SocketAddr,
        req: &RwReq,
        config: ServerConfig,
    ) -> Result<WriteRequest<'w, W>> {
        let oack_opts = build_oack_opts(&config, req);

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

        Ok(WriteRequest {
            peer,
            socket: UdpSocket::bind("0.0.0.0:0").await.map_err(Error::Bind)?,
            writer,
            buffer: BytesMut::new(),
            ack: BytesMut::new(),
            block_size,
            timeout,
            max_retries: config.max_send_retries,
            oack_opts,
        })
    }

    pub(crate) async fn handle(&mut self) {
        if let Err(e) = self.try_handle().await {
            log!("WRQ request failed (peer: {}, error: {}", self.peer, &e);

            Packet::Error(e.into()).encode(&mut self.buffer);
            let buf = self.buffer.split().freeze();
            // Errors are never retransmitted.
            // We do not care if `send_to` resulted to an IO error.
            let _ = self.socket.send_to(&buf[..], self.peer).await;
        }
    }

    async fn try_handle(&mut self) -> Result<()> {
        let mut block_id: u16 = 0;

        // Send first Ack/OAck
        match self.oack_opts.take() {
            Some(opts) => Packet::OAck(opts).encode(&mut self.ack),
            None => Packet::Ack(0).encode(&mut self.ack),
        }

        self.socket.send_to(&self.ack, self.peer).await?;

        loop {
            // Recv data
            block_id = block_id.wrapping_add(1);
            let data = self.recv_data(block_id).await?;

            // Write data to file
            self.writer.write_all(&data[..]).await?;

            if data.len() < self.block_size {
                break;
            }
        }

        Ok(())
    }

    async fn recv_data(&mut self, block_id: u16) -> Result<Bytes> {
        for _ in 0..=self.max_retries {
            match self.recv_data_block(block_id).await {
                Ok(data) => {
                    // Data received, send ACK
                    self.ack.clear();
                    Packet::Ack(block_id).encode(&mut self.ack);

                    self.socket.send_to(&self.ack, self.peer).await?;
                    return Ok(data);
                }
                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                    // On timeout reply with the previous ACK packet
                    self.socket.send_to(&self.ack, self.peer).await?;
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }

        Err(Error::MaxSendRetriesReached(self.peer, block_id))
    }

    async fn recv_data_block(&mut self, block_id: u16) -> io::Result<Bytes> {
        let socket = &mut self.socket;
        let peer = self.peer;

        self.buffer.resize(PACKET_DATA_HEADER_LEN + self.block_size, 0);
        let mut buf = self.buffer.split();

        io_timeout(self.timeout, async move {
            loop {
                let (len, recved_peer) = socket.recv_from(&mut buf[..]).await?;

                if recved_peer != peer {
                    continue;
                }

                if let Ok(Packet::Data(recved_block_id, _)) =
                    Packet::decode(&buf[..len])
                {
                    if recved_block_id == block_id {
                        buf.truncate(len);
                        buf.advance(PACKET_DATA_HEADER_LEN);
                        break;
                    }
                }
            }

            Ok(buf.freeze())
        })
        .await
    }
}

fn build_oack_opts(config: &ServerConfig, req: &RwReq) -> Option<Opts> {
    let mut opts = Opts::default();

    if !config.ignore_client_block_size {
        opts.block_size = match (req.opts.block_size, config.block_size_limit) {
            (Some(bsize), Some(limit)) => Some(cmp::min(bsize, limit)),
            (Some(bsize), None) => Some(bsize),
            _ => None,
        };
    }

    if !config.ignore_client_timeout {
        opts.timeout = req.opts.timeout;
    }

    opts.transfer_size = req.opts.transfer_size;

    if opts == Opts::default() {
        None
    } else {
        Some(opts)
    }
}
