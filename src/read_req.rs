use futures::{select, FutureExt};
use runtime::net::UdpSocket;
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::time::Duration;

use crate::error::*;
use crate::packet::*;
use crate::util::timeout;

const DEFAULT_TIMEOUT_SECS: u64 = 3;

pub struct ReadRequest {
    peer: SocketAddr,
    req: RwReq,
    socket: UdpSocket,
    block_id: u16,
}

impl ReadRequest {
    pub fn init(peer: SocketAddr, req: RwReq) -> Result<Self> {
        Ok(ReadRequest {
            peer,
            req,
            socket: UdpSocket::bind("0.0.0.0:0")?,
            block_id: 0,
        })
    }

    pub async fn handle(&mut self) -> Result<()> {
        let mut file = File::open(&self.req.filename)?;

        loop {
            let block = read_block(&mut file, 512)?;
            let last_block = block.len() < 512;

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
        let timeout_dur = match self.req.opts.timeout.unwrap_or(0) {
            0 => Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            secs => Duration::from_secs(secs as u64),
        };

        loop {
            self.socket.send_to(&packet[..], self.peer).await?;

            let mut recv_ack_fut = self.recv_ack().boxed().fuse();
            let mut timeout_fut = timeout(timeout_dur);

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
}

fn read_block(reader: &mut Read, block_size: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; block_size];
    let mut len = 0;

    while len < block_size {
        match reader.read(&mut buf[len..])? {
            0 => break,
            x => len += x,
        }
    }

    buf.truncate(len);
    Ok(buf)
}
