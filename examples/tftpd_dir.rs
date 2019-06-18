#![feature(async_await)]

use futures::io::AllowStdIo;
use std::fs::File;
use tftp::AsyncTftpServer;

struct Handler {}

impl Handler {
    fn new() -> Self {
        Handler {}
    }
}

impl tftp::ReadHandle for Handler {
    // Note that `AllowStdIo` is synchronous and makes event loop to block.
    // If you want to convert a synchronous to trully asynchronous, you can use
    // crates such as `sluice`.
    type Reader = AllowStdIo<File>;

    fn open(
        &mut self,
        path: &str,
    ) -> tftp::Result<(Self::Reader, Option<u64>)> {
        let file = File::open(path)?;
        let len = file.metadata().ok().map(|m| m.len());
        Ok((AllowStdIo::new(file), len))
    }
}

#[runtime::main]
async fn main() -> Result<(), tftp::Error> {
    let tftpd = AsyncTftpServer::bind(Handler::new(), "0.0.0.0:6969")?;
    println!("Listening on: {}", tftpd.local_addr()?);
    tftpd.serve().await
}
