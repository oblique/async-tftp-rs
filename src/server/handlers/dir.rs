use std::fs;
use std::net::SocketAddr;
use std::path::Component;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::packet;
use crate::runtime::File;

/// Handler that serves read requests for a directory.
pub struct DirHandler {
    dir: PathBuf,
    serve_rrq: bool,
    serve_wrq: bool,
}

pub enum DirHandlerFlags {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

impl DirHandler {
    /// Create new handler for directory.
    pub fn new<P>(dir: P, flags: DirHandlerFlags) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let dir = fs::canonicalize(dir.as_ref())?;

        if !dir.is_dir() {
            return Err(Error::NotDir(dir));
        }

        log!("TFTP directory: {}", dir.display());

        let serve_rrq = match flags {
            DirHandlerFlags::ReadOnly => true,
            DirHandlerFlags::WriteOnly => false,
            DirHandlerFlags::ReadWrite => true,
        };

        let serve_wrq = match flags {
            DirHandlerFlags::ReadOnly => false,
            DirHandlerFlags::WriteOnly => true,
            DirHandlerFlags::ReadWrite => true,
        };

        Ok(DirHandler {
            dir,
            serve_rrq,
            serve_wrq,
        })
    }
}

#[crate::async_trait]
impl crate::server::Handler for DirHandler {
    type Reader = File;
    type Writer = File;

    async fn read_req_open(
        &mut self,
        _client: &SocketAddr,
        path: &Path,
    ) -> Result<(Self::Reader, Option<u64>), packet::Error> {
        if !self.serve_rrq {
            return Err(packet::Error::IllegalOperation);
        }

        let path = secure_path(&self.dir, path)?;

        // Send only regular files
        if !path.is_file() {
            return Err(packet::Error::FileNotFound);
        }

        let file = File::open(&path).await?;
        let len = file.metadata().await.ok().map(|m| m.len());

        log!("TFTP sending file: {}", path.display());

        Ok((file, len))
    }

    async fn write_req_open(
        &mut self,
        _client: &SocketAddr,
        path: &Path,
        size: Option<u64>,
    ) -> Result<Self::Writer, packet::Error> {
        if !self.serve_wrq {
            return Err(packet::Error::IllegalOperation);
        }

        let path = secure_path(&self.dir, path)?;
        let file = File::create(path).await?;

        if let Some(size) = size {
            file.set_len(size).await?;
        }

        Ok(file)
    }
}

fn secure_path(
    restricted_dir: &Path,
    path: &Path,
) -> Result<PathBuf, packet::Error> {
    // Strip `/` and `./` prefixes
    let path = path
        .strip_prefix("/")
        .or_else(|_| path.strip_prefix("./"))
        .unwrap_or(path);

    // Avoid directory traversal attack by filtering `../`.
    if path.components().any(|x| x == Component::ParentDir) {
        return Err(packet::Error::PermissionDenied);
    }

    // Path should not start from root dir or have any Windows prefixes.
    // i.e. We accept only normal path components.
    match path.components().next() {
        Some(Component::Normal(_)) => {}
        _ => return Err(packet::Error::PermissionDenied),
    }

    Ok(restricted_dir.join(path))
}
