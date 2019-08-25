use bytes::{Bytes, BytesMut};
use futures::io::{AsyncWrite, AsyncWriteExt};
use std::net::SocketAddr;

use crate::error::*;
use crate::packet::*;
use crate::wrappers::{udp_socket_bind, UdpSocket};

pub struct WriteRequest<W>
where
    W: AsyncWrite + Send,
{
    peer: SocketAddr,
    _req: RwReq,
    socket: UdpSocket,
    block_id: u16,
    writer: W,
}

impl<W> WriteRequest<W>
where
    W: AsyncWrite + Send + Unpin,
{
    pub async fn init(writer: W, peer: SocketAddr, req: RwReq) -> Result<Self> {
        Ok(WriteRequest {
            peer,
            _req: req,
            socket: udp_socket_bind("0.0.0.0:0").await.map_err(Error::Bind)?,
            block_id: 0,
            writer,
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
        let ack = Packet::Ack(self.block_id).to_bytes();
        self.socket.send_to(&ack[..], self.peer).await?;

        loop {
            // recv data
            self.block_id = self.block_id.wrapping_add(1);
            let data = self.recv_data().await?;

            // write data
            self.writer.write_all(&data[..]).await?;

            // ack
            let ack = Packet::Ack(self.block_id).to_bytes();
            // TODO: resend on timeout
            self.socket.send_to(&ack[..], self.peer).await?;

            if data.len() < 512 {
                break;
            }
        }

        Ok(())
    }

    async fn recv_data(&mut self) -> Result<Bytes> {
        let mut buf = BytesMut::new();
        buf.resize(4096, 0);

        let (data_pos, data_len) = loop {
            let (len, peer) = self.socket.recv_from(&mut buf[..]).await?;

            // ignore packets from any other peers
            if peer != self.peer {
                continue;
            }

            let packet = Packet::from_bytes(&buf[..len])?;

            // TODO: handle Packet::Error as error::Error
            if let Packet::Data(block_id, data) = packet {
                // ignore packet with wrong block id
                if block_id != self.block_id {
                    continue;
                }

                // position of data within the original buffer
                let data_pos = data.as_ptr() as usize - buf.as_ptr() as usize;
                break (data_pos, data.len());
            }
        };

        buf.advance(data_pos);
        buf.truncate(data_len);

        Ok(buf.freeze())
    }
}
