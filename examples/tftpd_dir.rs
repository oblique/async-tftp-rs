#![feature(async_await)]

use futures::io::AllowStdIo;
use simplelog::{Config, LevelFilter, TermLogger, TerminalMode};
use std::fs::File;
use tftp::AsyncTftpServer;

struct Handler {}

impl Handler {
    fn new() -> Self {
        Handler {}
    }
}

impl tftp::Handle for Handler {
    // Note that `AllowStdIo` is synchronous and makes event loop to block.
    // If you want to convert a synchronous to trully asynchronous, you can use
    // crates such as `sluice`.
    type Reader = AllowStdIo<File>;
    type Writer = AllowStdIo<File>;

    fn read_open(
        &mut self,
        path: &str,
    ) -> tftp::Result<(Self::Reader, Option<u64>)> {
        // Avoid directory traversal attacks
        if path.contains("..") {
            return Err(tftp::Error::InvalidOperation);
        }

        let file = File::open(path)?;
        let len = file.metadata().ok().map(|m| m.len());
        Ok((AllowStdIo::new(file), len))
    }

    fn write_open(
        &mut self,
        path: &str,
        _size: Option<u64>,
    ) -> tftp::Result<Self::Writer> {
        // Avoid directory traversal attacks
        if path.contains("..") {
            return Err(tftp::Error::InvalidOperation);
        }

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

    let tftpd = AsyncTftpServer::bind(Handler::new(), "0.0.0.0:6969")?;
    println!("Listening on: {}", tftpd.local_addr()?);

    tftpd.serve().await
}
