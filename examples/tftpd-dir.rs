use anyhow::Result;
use async_tftp::server::TftpServerBuilder;
use simplelog::{Config, LevelFilter, TermLogger, TerminalMode};

fn main() -> Result<()> {
    smol::run(async {
        // Init logger
        TermLogger::init(
            LevelFilter::Info,
            Config::default(),
            TerminalMode::Stdout,
        )?;

        async_tftp::log::set_log_level(log::Level::Info);

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
