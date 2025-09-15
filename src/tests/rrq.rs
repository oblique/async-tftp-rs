#![cfg(feature = "external-client-tests")]
#![cfg(target_os = "linux")]

use async_executor::Executor;
use blocking::Unblock;
use futures_lite::future::block_on;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use super::external_client::*;
use super::handlers::*;
use crate::server::TftpServerBuilder;

fn transfer(
    file_size: usize,
    block_size: Option<u16>,
    server_window_size: Option<u16>,
    client_window_size: Option<u16>,
) {
    let ex = Arc::new(Executor::new());
    let transfered = Rc::new(Cell::new(false));

    block_on(ex.run({
        let ex = ex.clone();
        let transfered = transfered.clone();

        async move {
            let (md5_tx, md5_rx) = async_channel::bounded(1);
            let handler = RandomHandler::new(file_size, md5_tx);

            // bind
            let tftpd = TftpServerBuilder::with_handler(handler)
                .bind("127.0.0.1:0".parse().unwrap())
                .window_size_limit(server_window_size.unwrap_or(1))
                .build()
                .await
                .unwrap();
            let addr = tftpd.listen_addr().unwrap();

            // start client
            let mut tftp_recv = Unblock::new(());
            let tftp_recv = tftp_recv.with_mut(move |_| {
                external_tftp_recv("test", addr, block_size, client_window_size)
            });

            // start server
            ex.spawn(async move {
                tftpd.serve().await.unwrap();
            })
            .detach();

            // check md5
            let client_md5 =
                tftp_recv.await.expect("failed to run tftp client");
            let server_md5 =
                md5_rx.recv().await.expect("failed to receive server md5");
            assert_eq!(client_md5, server_md5);

            transfered.set(true);
        }
    }));

    assert!(transfered.get());
}
#[test]
fn transfer_0_bytes() {
    transfer(0, None, None, None);
    transfer(0, Some(1024), None, None);
    transfer(0, Some(1024), Some(8), Some(8));
}

#[test]
fn transfer_less_than_block() {
    transfer(1, None, None, None);
    transfer(123, None, None, None);
    transfer(511, None, None, None);
    transfer(1023, Some(1024), None, None);
    transfer(1, None, Some(8), Some(8));
    transfer(123, None, Some(8), Some(8));
    transfer(511, None, Some(8), Some(8));
    transfer(1023, Some(1024), Some(8), Some(8));
}

#[test]
fn transfer_block() {
    transfer(512, None, None, None);
    transfer(1024, Some(1024), None, None);
    transfer(1024, Some(1024), Some(8), Some(8));
}

#[test]
fn transfer_more_than_block() {
    transfer(512 + 1, None, None, None);
    transfer(512 + 123, None, None, None);
    transfer(512 + 511, None, None, None);
    transfer(1024 + 1, Some(1024), None, None);
    transfer(1024 + 123, Some(1024), None, None);
    transfer(1024 + 1023, Some(1024), None, None);
    transfer(1024 + 1023, Some(1024), Some(8), Some(4));
}

#[test]
fn transfer_1mb() {
    transfer(1024 * 1024, None, None, None);
    transfer(1024 * 1024, Some(1024), None, None);
    transfer(1024 * 1024, Some(1024), Some(16), Some(8));
}

#[test]
#[ignore]
fn transfer_almost_32mb() {
    transfer(32 * 1024 * 1024 - 1, None, None, None);

    for window_size in
        [None, Some(1), Some(2), Some(3), Some(4), Some(5), Some(8), Some(16)]
    {
        transfer(32 * 1024 * 1024 - 1, None, window_size, window_size);
    }
}

#[test]
#[ignore]
fn transfer_32mb() {
    transfer(32 * 1024 * 1024, None, None, None);
}

#[test]
#[ignore]
fn transfer_more_than_32mb() {
    transfer(33 * 1024 * 1024 + 123, None, None, None);
}

#[test]
#[ignore]
fn transfer_more_than_64mb() {
    transfer(65 * 1024 * 1024 + 123, None, None, None);
}
