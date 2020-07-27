use blocking::{unblock, Unblock};
use log::trace;
use std::fs::{self, File};
use std::io;
use std::net::SocketAddr;
use std::path::Component;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::packet;

/// Handler that serves read requests for a directory.
pub struct DirHandler {
    dir: PathBuf,
    serve_rrq: bool,
    serve_wrq: bool,
}

pub enum DirHandlerMode {
    /// Serve only read requests.
    ReadOnly,
    /// Serve only write requests.
    WriteOnly,
    /// Server read and write requests.
    ReadWrite,
}

impl DirHandler {
    /// Create new handler for directory.
    pub fn new<P>(dir: P, flags: DirHandlerMode) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let dir = fs::canonicalize(dir.as_ref())?;

        if !dir.is_dir() {
            return Err(Error::NotDir(dir));
        }

        trace!("TFTP directory: {}", dir.display());

        let serve_rrq = match flags {
            DirHandlerMode::ReadOnly => true,
            DirHandlerMode::WriteOnly => false,
            DirHandlerMode::ReadWrite => true,
        };

        let serve_wrq = match flags {
            DirHandlerMode::ReadOnly => false,
            DirHandlerMode::WriteOnly => true,
            DirHandlerMode::ReadWrite => true,
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
    type Reader = Unblock<File>;
    type Writer = Unblock<File>;

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

        let path_clone = path.clone();
        let (file, len) = unblock!(open_file_ro(path_clone))?;
        let reader = Unblock::new(file);

        trace!("TFTP sending file: {}", path.display());

        Ok((reader, len))
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

        let path_clone = path.clone();
        let file = unblock!(open_file_wo(path_clone, size))?;
        let writer = Unblock::new(file);

        trace!("TFTP receiving file: {}", path.display());

        Ok(writer)
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

fn open_file_ro(path: PathBuf) -> io::Result<(File, Option<u64>)> {
    let file = File::open(&path)?;
    let len = file.metadata().ok().map(|m| m.len());
    Ok((file, len))
}

fn open_file_wo(path: PathBuf, size: Option<u64>) -> io::Result<File> {
    let file = File::create(path)?;

    if let Some(size) = size {
        file.set_len(size)?;
    }

    Ok(file)
}
