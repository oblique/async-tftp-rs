use anyhow::Result;
use simplelog::{Config, LevelFilter, TermLogger, TerminalMode};
use structopt::StructOpt;

use async_tftp::packet;
use async_tftp::server::{Handler, TftpServerBuilder};
use flate2::read::GzDecoder;
use futures::channel::oneshot;
use futures::io;
use std::fs::File;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tar::Archive;
use threadpool::ThreadPool;

struct TftpdTarGzHandler {
    archive_path: PathBuf,
    thread_pool: ThreadPool,
}

impl TftpdTarGzHandler {
    fn new(path: impl AsRef<Path>) -> Self {
        TftpdTarGzHandler {
            archive_path: path.as_ref().to_owned(),
            thread_pool: ThreadPool::new(5),
        }
    }
}

// Sometimes paths within archives start with `/` or `./`, strip both.
fn strip_path_prefixes(path: &Path) -> &Path {
    path.strip_prefix("/").or_else(|_| path.strip_prefix("./")).unwrap_or(path)
}

// This macro sends `Err` over a channel, it is used within `thread_pool`.
macro_rules! try_or_send {
    ($e:expr, $tx:ident) => {{
        match $e {
            Ok(x) => x,
            Err(e) => {
                let _ = $tx.send(Err(e.into()));
                return;
            }
        }
    }};
}

#[async_tftp::async_trait]
impl Handler for TftpdTarGzHandler {
    type Reader = piper::Reader;
    type Writer = io::Sink;

    async fn read_req_open(
        &mut self,
        _client: &SocketAddr,
        path: &Path,
    ) -> Result<(Self::Reader, Option<u64>), packet::Error> {
        let req_path = strip_path_prefixes(path).to_owned();
        let archive_path = self.archive_path.clone();

        let (pipe_r, mut pipe_w) = piper::pipe(65536);
        let (open_res_tx, open_res_rx) = oneshot::channel();

        // We need to use our own thread pool to handle blocking IO
        // of `tar::Entry`.
        self.thread_pool.execute(move || {
            let file = try_or_send!(File::open(archive_path), open_res_tx);

            let mut archive = Archive::new(GzDecoder::new(file));
            let entries = try_or_send!(archive.entries(), open_res_tx);

            for entry in entries {
                let entry = try_or_send!(entry, open_res_tx);

                // If entry path is the same with requested path.
                if entry
                    .path()
                    .map(|p| strip_path_prefixes(&p) == req_path)
                    .unwrap_or(false)
                {
                    // We manage to find the entry.

                    // Check if it is a regular file.
                    if entry.header().entry_type() != tar::EntryType::Regular {
                        break;
                    }

                    // Inform handler to continue on serving the data.
                    if open_res_tx.send(Ok(())).is_err() {
                        // Do not transfer data if handler task is canceled.
                        return;
                    }

                    // Forward data to handler.
                    let entry = io::AllowStdIo::new(entry);
                    let _ = smol::block_on(io::copy(entry, &mut pipe_w));

                    return;
                }
            }

            // Requested path not found within the archive.
            let _ = open_res_tx.send(Err(packet::Error::FileNotFound));
        });

        // Wait for the above task to find the requested path and
        // starts transferring data.
        open_res_rx.await.unwrap_or(Err(packet::Error::FileNotFound))?;

        Ok((pipe_r, None))
    }

    async fn write_req_open(
        &mut self,
        _client: &SocketAddr,
        _path: &Path,
        _size: Option<u64>,
    ) -> Result<Self::Writer, packet::Error> {
        Err(packet::Error::IllegalOperation)
    }
}

#[derive(Debug, StructOpt)]
struct Opt {
    archive_path: PathBuf,
}

fn main() -> Result<()> {
    // Parse args
    let opt = Opt::from_args();

    // Init logger
    TermLogger::init(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Stdout,
    )?;

    async_tftp::log::set_log_level(log::Level::Info);

    smol::run(async move {
        // We will serve files from a tar.gz through tftp
        let handler = TftpdTarGzHandler::new(&opt.archive_path);

        // Build server
        let tftpd = TftpServerBuilder::with_handler(handler)
            .bind("0.0.0.0:6969".parse().unwrap())
            // Workaround to handle cases where client is behind VPN
            .block_size_limit(1024)
            .build()
            .await?;

        // Serve
        log::info!("Listening on: {}", tftpd.listen_addr()?);
        tftpd.serve().await?;

        Ok(())
    })
}
