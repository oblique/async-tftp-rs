use anyhow::Result;
use async_tftp::server::TftpServerBuilder;
use tracing::{trace, Level};
use tracing_subscriber::FmtSubscriber;

async fn run() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_writer(std::io::stderr)
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    let tftpd = TftpServerBuilder::with_dir_ro(".")?
        .bind("0.0.0.0:6969".parse().unwrap())
        // Workaround to handle cases where client is behind VPN
        .maximum_block_size(1024)
        .build()
        .await?;

    trace!("Listening on: {}", tftpd.local_addr()?);
    tftpd.serve().await?;

    Ok(())
}

fn main() -> Result<()> {
    async_std::task::block_on(run())
}
