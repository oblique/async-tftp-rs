use futures::io::Sink;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::random_file::RandomFile;
use crate::packet;
use crate::server::Handle;

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

#[crate::async_trait]
impl Handle for RandomHandle {
    type Reader = RandomFile;
    #[cfg(feature = "unstable")]
    type Writer = Sink;

    async fn read_req_open(
        &mut self,
        _client: &SocketAddr,
        _path: &Path,
    ) -> Result<(Self::Reader, Option<u64>), packet::Error> {
        Ok((RandomFile::new(self.file_size), None))
    }

    async fn read_req_served(
        &mut self,
        _client: &SocketAddr,
        _path: &Path,
        reader: Self::Reader,
    ) {
        let mut md5 = self.md5.lock().unwrap();
        *md5 = Some(reader.hash());
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
