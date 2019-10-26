use async_trait::async_trait;
use futures::io::Sink;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::random_file::RandomFile;
use crate::Handle;
use crate::TftpError;

pub struct RandomHandle {
    md5: Arc<Mutex<Option<md5::Digest>>>,
    file_size: usize,
}

impl RandomHandle {
    pub fn new(file_size: usize) -> Self {
        RandomHandle {
            md5: Arc::new(Mutex::new(None)),
            file_size,
        }
    }

    pub fn md5(&self) -> Arc<Mutex<Option<md5::Digest>>> {
        self.md5.clone()
    }
}

#[async_trait]
impl Handle for RandomHandle {
    type Reader = RandomFile;
    type Writer = Sink;

    async fn read_open(
        &mut self,
        _client: &SocketAddr,
        _path: &Path,
    ) -> Result<(Self::Reader, Option<u64>), TftpError> {
        Ok((RandomFile::new(self.file_size), None))
    }

    async fn rrq_served(
        &mut self,
        _client: &SocketAddr,
        _path: &Path,
        reader: Self::Reader,
    ) {
        let mut md5 = self.md5.lock().unwrap();
        *md5 = Some(reader.hash());
    }

    async fn write_open(
        &mut self,
        _client: &SocketAddr,
        _path: &Path,
        _size: Option<u64>,
    ) -> Result<Self::Writer, TftpError> {
        Err(TftpError::IllegalOperation)
    }
}
