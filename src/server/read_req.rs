#![allow(clippy::transmute_ptr_to_ptr)]

use async_net::UdpSocket;
use bytes::{BufMut, Bytes, BytesMut};
use futures_lite::{AsyncRead, AsyncReadExt};
use std::cmp;
use std::io;
use std::mem;
use std::net::SocketAddr;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::packet::{Opts, Packet, RwReq, PACKET_DATA_HEADER_LEN};
use crate::server::{ServerConfig, DEFAULT_BLOCK_SIZE};
use crate::utils::io_timeout;

pub(crate) struct ReadRequest<'r, R>
where
    R: AsyncRead + Send,
{
    peer: SocketAddr,
    socket: UdpSocket,
    reader: &'r mut R,
    buffer: BytesMut,
    block_size: usize,
    timeout: Duration,
    max_send_retries: u32,
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
            block_size,
            timeout,
            max_send_retries: config.max_send_retries,
            oack_opts,
        })
    }

    pub(crate) async fn handle(&mut self) {
        if let Err(e) = self.try_handle().await {
            log!("RRQ request failed (peer: {}, error: {})", &self.peer, &e);

            Packet::Error(e.into()).encode(&mut self.buffer);
            let buf = self.buffer.split().freeze();
            // Errors are never retransmitted.
            // We do not care if `send_to` resulted to an IO error.
            let _ = self.socket.send_to(&buf[..], self.peer).await;
        }
    }

    async fn try_handle(&mut self) -> Result<()> {
        let mut block_id: u16 = 0;

        // Send file to client
        loop {
            let is_last_block;

            // Reclaim buffer
            self.buffer.reserve(PACKET_DATA_HEADER_LEN + self.block_size);

            // Encode head of Data packet
            block_id = block_id.wrapping_add(1);
            Packet::encode_data_head(block_id, &mut self.buffer);

            // Read block in self.buffer
            let buf = unsafe {
                let data_buf: &mut [u8] =
                    mem::transmute(self.buffer.bytes_mut());

                let len = self.read_block(data_buf).await?;
                is_last_block = len < self.block_size;

                self.buffer.advance_mut(len);
                self.buffer.split().freeze()
            };

            // Send OACK after we manage to read the first block from reader.
            //
            // We do this because we want to give the developers the option to
            // produce an error after they construct a reader.
            if let Some(opts) = self.oack_opts.take() {
                log!("RRQ OACK (peer: {}, opts: {:?}", &self.peer, &opts);

                let mut buf = BytesMut::new();
                Packet::OAck(opts.to_owned()).encode(&mut buf);

                self.send(buf.split().freeze(), 0).await?;
            }

            // Send Data packet
            self.send(buf, block_id).await?;

            if is_last_block {
                break;
            }
        }

        log!("RRQ request served (peer: {})", &self.peer);
        Ok(())
    }

    async fn send(&mut self, packet: Bytes, block_id: u16) -> Result<()> {
        // Send packet until we receive an ack
        for _ in 0..=self.max_send_retries {
            self.socket.send_to(&packet[..], self.peer).await?;

            match self.recv_ack(block_id).await {
                Ok(_) => {
                    log!(
                        "RRQ (peer: {}, block_id: {}) - Received ACK",
                        &self.peer,
                        block_id
                    );
                    return Ok(());
                }
                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                    log!(
                        "RRQ (peer: {}, block_id: {}) - Timeout",
                        &self.peer,
                        block_id
                    );
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }

        Err(Error::MaxSendRetriesReached(self.peer, block_id))
    }

    async fn recv_ack(&mut self, block_id: u16) -> io::Result<()> {
        // We can not use `self` within `async_std::io::timeout` because not all
        // struct members implement `Sync`. So we borrow only what we need.
        let socket = &mut self.socket;
        let peer = self.peer;

        io_timeout(self.timeout, async {
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
        opts.block_size = match (req.opts.block_size, config.block_size_limit) {
            (Some(bsize), Some(limit)) => Some(cmp::min(bsize, limit)),
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
