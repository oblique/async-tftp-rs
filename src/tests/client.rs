use crate::packet::Error::OptionNegotiationFailed;
use crate::packet::{self, Mode, Opts, RwReq};
use crate::server::TftpServerBuilder;
use async_executor::Executor;
use async_io::{Async, Timer};
use futures_lite::future::block_on;
use futures_lite::{future, AsyncRead};
use std::cell::Cell;
use std::io;
use std::net::UdpSocket;
use std::rc::Rc;
use std::sync::Arc;

use super::handlers::*;
use super::packet::packet_to_bytes;
use std::task::Poll;
use std::time::Duration;

struct ResultsReader {
    results: Vec<io::Result<Vec<u8>>>,
}

impl AsyncRead for ResultsReader {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(match self.get_mut().results.pop() {
            Some(Ok(result)) => {
                buf[..result.len()].copy_from_slice(&result);
                Ok(result.len())
            }
            Some(Err(err)) => Err(err),
            None => Err(io::ErrorKind::NotFound.into()),
        })
    }
}

#[test]
fn test_abort_on_read_error() {
    let ex = Arc::new(Executor::new());
    let transferred = Rc::new(Cell::new(false));

    block_on(ex.run({
        let ex = ex.clone();
        let transferred = transferred.clone();

        async move {
            let handler = ReaderHandler::new(ResultsReader { results: vec![Err(io::ErrorKind::InvalidInput.into())] }, Some(4));
            let tftpd = TftpServerBuilder::with_handler(handler)
                .bind("127.0.0.1:0".parse().unwrap())
                .timeout(Duration::from_secs(1))
                .build()
                .await
                .unwrap();
            let addr = tftpd.listen_addr().unwrap();
            // start server
            let server_task = ex.spawn(async move {
                tftpd.serve().await.unwrap();
            });

            let socket = Async::<UdpSocket>::bind(([127, 0, 0, 1], 0)).unwrap();
            // send rrq with transfer size to 0 to simulate a transfer size probe
            let req_opts = Opts {
                transfer_size: Some(0),
                ..Default::default()
            };
            let rrq = packet::Packet::Rrq(RwReq {
                filename: "abc".to_string(),
                mode: Mode::Octet,
                opts: req_opts,
            });
            socket.send_to(&packet_to_bytes(&rrq), addr).await.unwrap();

            // read the ack
            let mut buf = [0u8; 1024];
            let (len, peer) = socket.recv_from(&mut buf).await.unwrap();
            let response = packet::Packet::decode(&buf[..len]).unwrap();
            assert!(matches!(
                    response,
                    packet::Packet::OAck(Opts {transfer_size: Some(4),..})),
                "Server did not send OAck packet: {:?}", response);

            // send error packet
            let abort_packet = packet::Packet::Error(OptionNegotiationFailed);
            socket.send_to(&packet_to_bytes(&abort_packet), peer).await.unwrap();

            // make sure the server doesn't send anything else
            assert!(
                future::race(
                    async move {
                        Timer::after(Duration::from_secs(3)).await;
                        true
                    },
                    async move {
                        // fail if we get anything after sending OptionNegotiationFailed error
                        let _ = socket.recv_from(&mut buf).await.unwrap();
                        false
                    }
                )
                    .await,
                "Server sent data after client sent OptionNegotiationFailed error"
            );
            server_task.cancel().await;
            transferred.set(true);
        }
        }));

    assert!(transferred.get());
}
