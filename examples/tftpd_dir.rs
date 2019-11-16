use anyhow::Result;
use async_std::fs::File;
use async_tftp::TftpError;
use async_tftp::TftpServerBuilder;
use std::net::SocketAddr;
use std::path::Path;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

struct Handler {}

impl Handler {
    fn new() -> Self {
        Handler {}
    }
}

#[async_tftp::async_trait]
impl async_tftp::Handle for Handler {
    type Reader = File;
    #[cfg(feature = "unstable")]
    type Writer = File;

    async fn read_req_open(
        &mut self,
        _client: &SocketAddr,
        path: &Path,
    ) -> Result<(Self::Reader, Option<u64>), TftpError> {
        let file = File::open(path).await?;
        let len = file.metadata().await.ok().map(|m| m.len());
        Ok((file, len))
    }

    #[cfg(feature = "unstable")]
    async fn write_open(
        &mut self,
        _client: &SocketAddr,
        path: &Path,
        _size: Option<u64>,
    ) -> Result<Self::Writer, TftpError> {
        let file = File::create(path).await?;
        Ok(file)
    }
}

async fn run() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_writer(std::io::stderr)
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    let tftpd = TftpServerBuilder::new(Handler::new())
        .bind("0.0.0.0:6969".parse().unwrap())
        // Workaround to handle cases that client is behind VPN
        .maximum_block_size(1024)
        .build()
        .await?;

    info!("Listening on: {}", tftpd.local_addr()?);
    tftpd.serve().await?;

    Ok(())
}

fn main() -> Result<()> {
    async_std::task::block_on(run())
}
