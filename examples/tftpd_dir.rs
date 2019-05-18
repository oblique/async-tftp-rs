#![feature(async_await)]

use tftp::AsyncTftpServer;

#[runtime::main]
async fn main() -> Result<(), tftp::Error> {
    let mut tftpd = AsyncTftpServer::bind("0.0.0.0:4444")?;

    println!("Listening on: {}", tftpd.local_addr()?);

    tftpd.serve().await
}
