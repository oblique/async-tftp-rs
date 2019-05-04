use futures::future;
use futures::future::{Future, FutureResult};
use futures::stream::Stream;
use std::io::{Read, Write};
use std::net::SocketAddr;
use tokio::net::{UdpFramed, UdpSocket};

use crate::codec::*;
use crate::error::*;
use crate::packet::*;

pub struct TftpServer<H: Handler> {
    handler: H,
    local_addr: SocketAddr,
    frame: Option<UdpFramed<Codec>>,
}

pub trait Handler {
    type Reader: Read;
    type Writer: Write;

    fn open_reader(
        &mut self,
        client: &SocketAddr,
        filename: &str,
        mode: Mode,
    ) -> Result<(Self::Reader, Option<usize>)>;

    fn open_writer(
        &mut self,
        client: &SocketAddr,
        filename: &str,
        mode: Mode,
        size: Option<usize>,
    ) -> Result<Self::Writer>;
}

impl<H> TftpServer<H>
where
    H: Handler,
{
    pub fn init(addr: SocketAddr, handler: H) -> Result<Self> {
        let socket = UdpSocket::bind(&addr)?;

        Ok(TftpServer {
            handler,
            local_addr: socket.local_addr()?,
            frame: Some(UdpFramed::new(socket, Codec::new())),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr.clone()
    }

    pub fn serve(&mut self) {
        self.frame
            .take()
            .expect("TftpServer not initialized correctly")
            .then(|res| -> Result<_> {
                match res {
                    Ok(x) => Ok(Some(x)),
                    // ignore invalid packets
                    Err(ref e) if e.kind() == &ErrorKind::InvalidPacket => {
                        Ok(None)
                    }
                    Err(e) => Err(e),
                }
            })
            .for_each(|res| {
                let (packet, addr) = match res {
                    Some(x) => x,
                    None => return future::ok(()),
                };

                match packet {
                    Packet::Rrq(file, mode, opts) => {
                        self.handle_rrq(addr, file, mode, opts)
                    }
                    Packet::Wrq(file, mode, opts) => {
                        self.handle_wrq(addr, file, mode, opts)
                    }
                    _ => future::ok(()),
                }
            })
            .wait()
            .unwrap();
    }

    pub fn handle_rrq(
        &mut self,
        client: SocketAddr,
        file: String,
        mode: Mode,
        opts: Opts,
    ) -> FutureResult<(), Error> {
        println!("rrq {:?} {:?} {:?}", file, mode, opts);
        future::ok(())
    }

    pub fn handle_wrq(
        &mut self,
        client: SocketAddr,
        file: String,
        mode: Mode,
        opts: Opts,
    ) -> FutureResult<(), Error> {
        println!("wrq {:?} {:?} {:?}", file, mode, opts);
        future::ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    struct Handle;

    impl Handler for Handle {
        type Writer = File;
        type Reader = File;

        fn open_reader(
            &mut self,
            client: &SocketAddr,
            filename: &str,
            mode: Mode,
        ) -> Result<(Self::Reader, Option<usize>)> {
            unimplemented!();
        }

        fn open_writer(
            &mut self,
            client: &SocketAddr,
            filename: &str,
            mode: Mode,
            size: Option<usize>,
        ) -> Result<Self::Writer> {
            unimplemented!();
        }
    }

    #[test]
    fn serve() {
        let mut s =
            TftpServer::init("0.0.0.0:4444".parse().unwrap(), Handle).unwrap();
        println!("{:?}", s.local_addr());
        //        s.serve();
    }
}
