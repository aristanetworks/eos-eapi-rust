// Copyright (c) 2022, Arista Networks, Inc.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are
// met:
//
//   * Redistributions of source code must retain the above copyright notice,
//   this list of conditions and the following disclaimer.
//
//   * Redistributions in binary form must reproduce the above copyright
//   notice, this list of conditions and the following disclaimer in the
//   documentation and/or other materials provided with the distribution.
//
//   * Neither the name of Arista Networks nor the names of its
//   contributors may be used to endorse or promote products derived from
//   this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL ARISTA NETWORKS
// BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR
// BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
// WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE
// OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN
// IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

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
    pub async fn connect<T1: AsRef<str>, T2: AsRef<str>>(
        sysname: T1,
        socket_name: T2,
    ) -> Result<Self, Error> {
        let sysname = sysname.as_ref().to_string();
        let socket_name = socket_name.as_ref().to_string();
        tokio::task::spawn_blocking(|| SyncClient::connect(sysname, socket_name))
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
