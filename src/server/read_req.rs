use async_io::Async;
use bytes::{BufMut, Bytes, BytesMut};
use futures_lite::{AsyncRead, AsyncReadExt};
use log::trace;
use std::cmp;
use std::collections::VecDeque;
use std::io;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::slice;
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
    socket: Async<UdpSocket>,
    reader: &'r mut R,
    block_size: usize,
    timeout: Duration,
    max_send_retries: u32,
    oack_opts: Option<Opts>,
    window_size: usize,
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
        local_ip: IpAddr,
    ) -> Result<ReadRequest<'r, R>> {
        let oack_opts = build_oack_opts(&config, req, file_size);

        let block_size = oack_opts
            .as_ref()
            .and_then(|o| o.block_size)
            .map(usize::from)
            .unwrap_or(DEFAULT_BLOCK_SIZE);

        // Default window size is 1 as per rfc7440
        let negotiated_window_size: usize =
            oack_opts.as_ref().and_then(|o| o.window_size).unwrap_or(1u16)
                as usize;

        let timeout = oack_opts
            .as_ref()
            .and_then(|o| o.timeout)
            .map(|t| Duration::from_secs(u64::from(t)))
            .unwrap_or(config.timeout);

        let addr = SocketAddr::new(local_ip, 0);
        let socket = Async::<UdpSocket>::bind(addr).map_err(Error::Bind)?;

        Ok(ReadRequest {
            peer,
            socket,
            reader,
            block_size,
            timeout,
            max_send_retries: config.max_send_retries,
            oack_opts,
            window_size: negotiated_window_size,
        })
    }

    pub(crate) async fn handle(&mut self) {
        if let Err(e) = self.try_handle().await {
            trace!("RRQ request failed (peer: {}, error: {})", &self.peer, &e);

            if let Error::Packet(
                crate::packet::Error::OptionNegotiationFailed,
            ) = e
            {
                // client aborted the connection, nothing to do
            } else {
                let mut buffer = BytesMut::with_capacity(DEFAULT_BLOCK_SIZE);
                Packet::Error(e.into()).encode(&mut buffer);
                let buf = buffer.split().freeze();
                // Errors are never retransmitted.
                // We do not care if `send_to` resulted to an IO error.
                let _ = self.socket.send_to(&buf[..], self.peer).await;
            }
        }
    }

    async fn try_handle(&mut self) -> Result<()> {
        let mut window: VecDeque<Bytes> =
            VecDeque::with_capacity(self.window_size);
        let mut block_id: u16;
        let mut window_base: u16 = 1;
        let mut buf: Bytes;
        let mut is_last_block: bool = false;

        if let Some(opts) = self.oack_opts.take() {
            trace!("RRQ OACK (peer: {}, opts: {:?}", &self.peer, &opts);
            // Send OACK after we manage to read the first block from the reader for
            // non-transfer size probe requests (oack.transfer_size value is set).
            // During transfer size probes a client aborts the connection after receiving
            // oack from the server. For normal requests we do this because we want to give
            // the developers the option to produce an error after they construct a reader.
            if opts.transfer_size.is_none() {
                (buf, is_last_block) =
                    self.fill_data_block(window_base).await?;
                window.push_back(buf);
            }
            let mut buff = BytesMut::with_capacity(PACKET_DATA_HEADER_LEN + 64);
            Packet::OAck(opts.to_owned()).encode(&mut buff);
            // OACK is not really part of the window, so we send it separately
            self.send_window(&VecDeque::from([buff.freeze()]), 0).await?;
        }

        loop {
            // calculate next block_id, window might not be empty
            block_id = window_base.wrapping_add(window.len() as u16);

            while !is_last_block && (window.len() < self.window_size) {
                // we still have data and window is not full
                (buf, is_last_block) = self.fill_data_block(block_id).await?;
                window.push_back(buf);
                block_id = block_id.wrapping_add(1);
            }

            let blocks_acked = self.send_window(&window, window_base).await?;
            window_base = window_base.wrapping_add(blocks_acked);

            // remove acked blocks from window
            if blocks_acked == window.len() as u16 {
                window.clear()
            } else {
                window.drain(..blocks_acked as usize);
            }

            if is_last_block && window.is_empty() {
                // transfer is done
                break;
            }
        }

        trace!("RRQ request served (peer: {})", &self.peer);
        Ok(())
    }

    async fn fill_data_block(
        &mut self,
        block_id: u16,
    ) -> Result<(Bytes, bool), Error> {
        let mut buffer: BytesMut =
            BytesMut::with_capacity(PACKET_DATA_HEADER_LEN + self.block_size);
        Packet::encode_data_head(block_id, &mut buffer);

        // Read block in buffer
        unsafe {
            let uninit_buf = buffer.chunk_mut();
            let data_buf = slice::from_raw_parts_mut(
                uninit_buf.as_mut_ptr(),
                uninit_buf.len(),
            );

            let len = self.read_block(data_buf).await?;
            buffer.advance_mut(len);
            Ok((buffer.split().freeze(), len < self.block_size))
        }
    }

    /// Sends packets contained in a window and waits for client to acknowledge them. Returns amount
    /// of packets acknowledged.
    async fn send_window(
        &mut self,
        window: &VecDeque<Bytes>,
        window_base: u16,
    ) -> Result<u16> {
        // Send packet until we receive an ack
        for _ in 0..=self.max_send_retries {
            for packet in window {
                self.socket.send_to(&packet[..], self.peer).await?;
            }

            match self.recv_ack(window_base, window.len() as u16).await {
                Ok(blocks_acked) => {
                    trace!(
                        "RRQ (peer: {}, window_base: {}, blocks_acked: {}, window_len: {}) - Received ACK",
                        &self.peer,
                        window_base,
                        blocks_acked,
                        window.len()
                    );
                    return Ok(blocks_acked);
                }
                Err(Error::Io(ref e))
                    if e.kind() == io::ErrorKind::TimedOut =>
                {
                    trace!(
                        "RRQ (peer: {}, block_id: {}) - Timeout",
                        &self.peer,
                        window_base
                    );
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(Error::MaxSendRetriesReached(self.peer, window_base))
    }

    /// Waits for ack packet, returns amount of packets acknowledged.
    async fn recv_ack(
        &mut self,
        window_base: u16,
        window_len: u16,
    ) -> Result<u16> {
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
                // if let Ok(Packet::Ack(recved_block_id)) =
                match Packet::decode(&buf[..len])
                {
                    Ok(Packet::Ack(recved_block_id)) => {
                        let window_end = window_base.wrapping_add(window_len);

                        if window_end > window_base {
                            // window_end did not wrap
                            if recved_block_id >= window_base && recved_block_id < window_end {
                                // number of blocks acked
                                return Ok(recved_block_id - window_base + 1u16);
                            } else {
                                trace!("Unexpected ack packet {recved_block_id}, window_base: {window_base}, window_len: {window_len}");
                            }
                        } else {
                            // window_end wrapped
                            if recved_block_id >= window_base {
                                return Ok(1u16 + (recved_block_id - window_base));
                            } else if recved_block_id < window_end {
                                return Ok(1u16 + recved_block_id + (window_len - window_end));
                            } else {
                                trace!("Unexpected ack packet {recved_block_id}, window_base: {window_base}, window_len: {window_len}");
                            }
                        }
                    },
                     Ok(Packet::Error(error)) if error.is_client_error()=> {
                         // pass errors coming from the client
                        return Err(Error::Packet(error))
                    }
                    // ignore all other errors
                    _ => {}
                }
            }
        })
        .await
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

    if !config.ignore_client_window_size {
        opts.window_size =
            match (req.opts.window_size, config.window_size_limit) {
                (Some(wsize), Some(limit)) => Some(cmp::min(wsize, limit)),
                (Some(wsize), None) => Some(wsize),
                _ => None,
            };
    }

    if opts == Opts::default() {
        None
    } else {
        Some(opts)
    }
}
