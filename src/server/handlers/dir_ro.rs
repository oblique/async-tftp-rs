use async_std::fs::File;
#[cfg(feature = "unstable")]
use futures::io::Sink;
use std::fs;
use std::net::SocketAddr;
use std::path::Component;
use std::path::{Path, PathBuf};

use crate::error::*;
use crate::packet;

pub struct DirRoHandler {
    dir: PathBuf,
}

impl DirRoHandler {
    pub fn new<P>(dir: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let dir = fs::canonicalize(dir.as_ref())?;

        if !dir.is_dir() {
            return Err(Error::NotDir(dir));
        }

        log!("TFTP directory: {}", dir.display());

        Ok(DirRoHandler {
            dir,
        })
    }
}

#[crate::async_trait]
impl crate::server::Handler for DirRoHandler {
    type Reader = File;
    #[cfg(feature = "unstable")]
    type Writer = Sink;

    async fn read_req_open(
        &mut self,
        _client: &SocketAddr,
        path: &Path,
    ) -> Result<(Self::Reader, Option<u64>), packet::Error> {
        // Strip some prefixes
        let path = path
            .strip_prefix("/")
            .or_else(|_| path.strip_prefix("./"))
            .unwrap_or(path);

        // Avoid directory traversal attack
        if path.components().any(|x| x == Component::ParentDir) {
            return Err(packet::Error::FileNotFound);
        }

        // Path should not start from root dir or have any Windows prefixes.
        // i.e. We accept only normal path components.
        match path.components().next() {
            Some(Component::Normal(_)) => {}
            _ => return Err(packet::Error::FileNotFound),
        }

        let path = self.dir.join(path);

        // Send only regular files
        if !path.is_file() {
            return Err(packet::Error::FileNotFound);
        }

        let file = File::open(&path).await?;
        let len = file.metadata().await.ok().map(|m| m.len());
        log!("TFTP sending file: {}", path.display());

        Ok((file, len))
    }

    #[cfg(feature = "unstable")]
    async fn write_open(
        &mut self,
        _client: &SocketAddr,
        _path: &Path,
        _size: Option<u64>,
    ) -> Result<Self::Writer, packet::Error> {
        Err(packet::Error::IllegalOperation)
    }
}
