use anyhow::Result;
use structopt::StructOpt;

use async_compression::futures::bufread::GzipDecoder;
use async_std::fs::File;
use async_std::io::{BufReader, Sink};
use async_std::path::{Path, PathBuf};
use async_std::stream::StreamExt;
use async_std::task::block_on;
use async_tar::{Archive, Entry};
use async_tftp::packet;
use async_tftp::server::{Handler, TftpServerBuilder};
use std::net::SocketAddr;

struct TftpdTarGzHandler {
    archive_path: PathBuf,
}

impl TftpdTarGzHandler {
    fn new(path: impl AsRef<Path>) -> Self {
        TftpdTarGzHandler {
            archive_path: path.as_ref().to_owned(),
        }
    }
}

// Sometimes paths within archives start with `/` or `./`, strip both.
fn strip_path_prefixes(path: &Path) -> &Path {
    path.strip_prefix("/").or_else(|_| path.strip_prefix("./")).unwrap_or(path)
}

#[async_tftp::async_trait]
impl Handler for TftpdTarGzHandler {
    type Reader = Entry<Archive<GzipDecoder<BufReader<File>>>>;
    type Writer = Sink;

    async fn read_req_open(
        &mut self,
        _client: &SocketAddr,
        path: &std::path::Path,
    ) -> Result<(Self::Reader, Option<u64>), packet::Error> {
        let req_path = strip_path_prefixes(path.into()).to_owned();

        let file = File::open(self.archive_path.clone()).await?;
        let archive = Archive::new(GzipDecoder::new(BufReader::new(file)));

        let mut entries = archive.entries()?;

        while let Some(Ok(entry)) = entries.next().await {
            if entry
                .path()
                .map(|p| strip_path_prefixes(&*p) == req_path)
                .unwrap_or(false)
            {
                // We manage to find the entry.

                // Check if it is a regular file.
                if entry.header().entry_type() != async_tar::EntryType::Regular
                {
                    break;
                }

                return Ok((entry, None));
            }
        }

        Err(packet::Error::FileNotFound)
    }

    async fn write_req_open(
        &mut self,
        _client: &SocketAddr,
        _path: &std::path::Path,
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

    fern::Dispatch::new()
        .level(log::LevelFilter::Info)
        .level_for("async_tftp", log::LevelFilter::Trace)
        .chain(std::io::stdout())
        .apply()
        .expect("Failed to initialize logger");

    block_on(async move {
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
