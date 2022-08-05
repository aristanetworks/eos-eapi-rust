use crate::{
    client::{Client as SyncClient, Error as SyncError},
    protocol,
};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};

#[derive(Debug, Error)]
pub enum Error {
    #[error("i/o error: {0}")]
    IO(#[from] std::io::Error),
    #[error("async runtime error: {0}")]
    Async(#[from] tokio::task::JoinError),
    #[error("incomplete write")]
    IncompleteWrite,
    #[error(transparent)]
    Sync(#[from] SyncError),
}

pub struct Client {
    request: UnixStream,
    response: UnixStream,
    _signal: UnixStream,
    _stats: UnixStream,
}

impl std::convert::TryFrom<SyncClient> for Client {
    type Error = Error;

    fn try_from(value: SyncClient) -> Result<Self, Self::Error> {
        Ok(Self {
            request: UnixStream::from_std(value.request)?,
            response: UnixStream::from_std(value.response)?,
            _signal: UnixStream::from_std(value._signal)?,
            _stats: UnixStream::from_std(value._stats)?,
        })
    }
}

impl Client {
    pub async fn connect<T: AsRef<str>>(sysname: T) -> Result<Self, Error> {
        let sysname = sysname.as_ref().to_owned();
        tokio::task::spawn_blocking(|| SyncClient::connect(sysname))
            .await??
            .try_into()
    }

    pub async fn do_request<T: AsRef<str>>(&mut self, request: T) -> Result<Vec<u8>, Error> {
        let request = request.as_ref();
        let mut buf = [0; 4];
        let n = self
            .request
            .write_vectored(&protocol::make_string(request.as_bytes(), &mut buf))
            .await?;
        if n < buf.len() + request.len() {
            return Err(Error::IncompleteWrite);
        }

        let mut buf = Vec::new();
        self.response.read_to_end(&mut buf).await?;

        Ok(buf)
    }
}
