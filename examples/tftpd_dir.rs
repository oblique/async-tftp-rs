use async_trait::async_trait;
use futures::io::AllowStdIo;
use simplelog::{Config, LevelFilter, TermLogger, TerminalMode};
use std::fs::File;
use tftp::AsyncTftpServer;
use tftp::TftpError;

struct Handler {}

impl Handler {
    fn new() -> Self {
        Handler {}
    }
}

#[async_trait]
impl tftp::Handle for Handler {
    // TODO: do not use AllowStdIo
    type Reader = AllowStdIo<File>;
    type Writer = AllowStdIo<File>;

    async fn read_open(
        &mut self,
        path: &str,
    ) -> Result<(Self::Reader, Option<u64>), TftpError> {
        let file = File::open(path)?;
        let len = file.metadata().ok().map(|m| m.len());
        Ok((AllowStdIo::new(file), len))
    }

    async fn write_open(
        &mut self,
        path: &str,
        _size: Option<u64>,
    ) -> Result<Self::Writer, TftpError> {
        let file = File::create(path)?;
        Ok(AllowStdIo::new(file))
    }
}

#[runtime::main]
async fn main() -> Result<(), tftp::Error> {
    let log_config = Config {
        filter_ignore: Some(&["mio", "romio"]),
        thread: Some(simplelog::Level::Error),
        ..Config::default()
    };

    let _ =
        TermLogger::init(LevelFilter::Trace, log_config, TerminalMode::Mixed);

    let tftpd = AsyncTftpServer::bind(Handler::new(), "0.0.0.0:6969").await?;
    println!("Listening on: {}", tftpd.local_addr()?);

    tftpd.serve().await
}
