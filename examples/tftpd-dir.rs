use anyhow::Result;
use async_tftp::server::TftpServerBuilder;
use futures_lite::future::block_on;

fn main() -> Result<()> {
    fern::Dispatch::new()
        .level(log::LevelFilter::Info)
        .level_for("async_tftp", log::LevelFilter::Trace)
        .chain(std::io::stdout())
        .apply()
        .expect("Failed to initialize logger");

    block_on(async {
        let tftpd = TftpServerBuilder::with_dir_ro(".")?
            .bind("0.0.0.0:6969".parse().unwrap())
            // Workaround to handle cases where client is behind VPN
            .block_size_limit(1024)
            .build()
            .await?;

        log::info!("Listening on: {}", tftpd.listen_addr()?);
        tftpd.serve().await?;

        Ok(())
    })
}
