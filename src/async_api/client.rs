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
    client::{Error as SyncError, UdsClient as SyncUdsClient},
    protocol,
};
use async_trait::async_trait;
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
    #[error("Invalid HTTP status {0}")]
    HttpStatus(hyper::StatusCode),
    #[error("HTTP encoding error: {0}")]
    HttpEncoding(#[from] hyper::http::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] hyper::Error),
    #[error("Timeout: {0}")]
    Timeout(#[from] tokio::time::error::Elapsed),
    #[error(transparent)]
    Sync(#[from] SyncError),
}

#[async_trait]
pub trait Requester {
    async fn do_request<T: AsRef<str> + Send>(self, request: T) -> Result<Vec<u8>, Error>;
}

#[doc(hidden)]
pub struct UdsClient {
    request: UnixStream,
    response: UnixStream,
    _signal: UnixStream,
    _stats: UnixStream,
}

impl std::convert::TryFrom<SyncUdsClient> for UdsClient {
    type Error = Error;

    fn try_from(value: SyncUdsClient) -> Result<Self, Self::Error> {
        Ok(Self {
            request: UnixStream::from_std(value.request)?,
            response: UnixStream::from_std(value.response)?,
            _signal: UnixStream::from_std(value._signal)?,
            _stats: UnixStream::from_std(value._stats)?,
        })
    }
}

impl UdsClient {
    pub async fn connect<T1: AsRef<str>, T2: AsRef<str>>(
        sysname: T1,
        socket_name: T2,
    ) -> Result<Self, Error> {
        let sysname = sysname.as_ref().to_string();
        let socket_name = socket_name.as_ref().to_string();
        tokio::task::spawn_blocking(|| SyncUdsClient::connect(sysname, socket_name))
            .await??
            .try_into()
    }
}

#[async_trait]
impl Requester for UdsClient {
    async fn do_request<T: AsRef<str> + Send>(mut self, request: T) -> Result<Vec<u8>, Error> {
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

#[doc(hidden)]
pub type HttpConnector = hyper::client::HttpConnector;
#[doc(hidden)]
pub type HttpsConnector = hyper_rustls::HttpsConnector<hyper::client::HttpConnector>;

#[doc(hidden)]
pub struct HttpClient<T> {
    client: hyper::Client<T, hyper::Body>,
    url: String,
    auth: Option<String>,
    timeout: std::time::Duration,
}

impl HttpClient<hyper::client::HttpConnector> {
    pub fn new_http(
        hostname: String,
        auth: Option<(String, String)>,
        timeout: std::time::Duration,
    ) -> Self {
        Self {
            client: hyper::Client::builder().build_http(),
            url: crate::client::make_url(hostname, false),
            auth: crate::client::make_auth_header(auth),
            timeout,
        }
    }
}

impl HttpClient<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>> {
    pub fn new_https(
        hostname: String,
        auth: Option<(String, String)>,
        timeout: std::time::Duration,
        insecure: bool,
    ) -> Self {
        let b = hyper_rustls::HttpsConnectorBuilder::new();
        let b = if insecure {
            b.with_tls_config(crate::client::make_insecure_tls_config())
        } else {
            b.with_native_roots()
        };
        let https = b.https_only().enable_http1().build();

        Self {
            client: hyper::Client::builder().build(https),
            url: crate::client::make_url(hostname, true),
            auth: crate::client::make_auth_header(auth),
            timeout,
        }
    }
}

#[async_trait]
impl<T> Requester for &HttpClient<T>
where
    T: hyper::client::connect::Connect + Clone + Send + Sync + 'static,
{
    async fn do_request<S: AsRef<str> + Send>(self, request: S) -> Result<Vec<u8>, Error> {
        let mut b = hyper::Request::post(&self.url).header("Content-type", "application/json");
        if let Some(v) = &self.auth {
            b = b.header("Authorization", v);
        };
        let req = b.body(hyper::Body::from(request.as_ref().to_owned()))?;

        let resp = tokio::time::timeout(self.timeout, self.client.request(req)).await??;
        if !resp.status().is_success() {
            return Err(Error::HttpStatus(resp.status()));
        }

        Ok(hyper::body::to_bytes(resp.into_body()).await?.to_vec())
    }
}
