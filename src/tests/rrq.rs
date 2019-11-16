#![cfg(feature = "external-client-tests")]

use async_std::task;

use super::external_client::*;
use super::handles::*;
use crate::server::TftpServerBuilder;

fn transfer(file_size: usize) {
    task::block_on(async {
        let handle = RandomHandle::new(file_size);
        let md5 = handle.md5();

        // bind
        let tftpd = TftpServerBuilder::with_handler(handle)
            .bind("127.0.0.1:0".parse().unwrap())
            .build()
            .await
            .unwrap();
        let addr = tftpd.local_addr().unwrap();

        // start client
        let tftp_recv =
            task::spawn_blocking(move || external_tftp_recv("test", addr));

        // start server
        tftpd.serve().await.unwrap();

        // check md5
        let recved_md5 = tftp_recv.await.expect("failed to run tftp client");
        let sent_md5 = md5.lock().unwrap().expect("no md5");
        assert_eq!(recved_md5, sent_md5);
    });
}

#[test]
fn transfer_0_bytes() {
    transfer(0);
}

#[test]
fn transfer_less_than_block() {
    transfer(1);
    transfer(123);
    transfer(511);
}

#[test]
fn transfer_block() {
    transfer(512);
}

#[test]
fn transfer_more_than_block() {
    transfer(512 + 1);
    transfer(512 + 123);
    transfer(512 + 511);
}

#[test]
fn transfer_1mb() {
    transfer(1024 * 1024);
}

#[test]
#[ignore]
fn transfer_almost_32mb() {
    transfer(32 * 1024 * 1024 - 1);
}

#[test]
#[ignore]
fn transfer_32mb() {
    transfer(32 * 1024 * 1024);
}

#[test]
#[ignore]
fn transfer_more_than_32mb() {
    transfer(33 * 1024 * 1024 + 123);
}

#[test]
#[ignore]
fn transfer_more_than_64mb() {
    transfer(65 * 1024 * 1024 + 123);
}
