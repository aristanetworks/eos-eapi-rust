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

//! This module contains the `async` version of the functions in the main module.
//! # Example using unix sockets
//! The UDS client is one shot, and `run` consumes the client.
//! ```no_run
//! let result = ClientBuilder::unix_socket()
//!                 .build_async()
//!                 .await?
//!                 .run(&["show clock", "show aliases"], ResultFormat::Json)
//!                 .await?;
//! match result {
//!     Response::Result(v) => println!("{v:?}"),
//!     Response::Error {
//!         message,
//!         code,
//!         errors,
//!     } => println!("error code: {code}, message: {message}, outputs: {errors:#?}"),
//! };
//! ```
//!
//! # Example using HTTP
//! The HTTP(S) client can be reused to run multiple sets of commands.
//! ```no_run
//! let result = ClientBuilder::unix_http("localhost")
//!                 .set_authentication("admin".to_owned(), "pass".to_owned())
//!                 .build_async()
//!                 .run(&["show clock", "show aliases"], ResultFormat::Json)
//!                 .await?;
//! match result {
//!     Response::Result(v) => println!("{v:?}"),
//!     Response::Error {
//!         message,
//!         code,
//!         errors,
//!     } => println!("error code: {code}, message: {message}, outputs: {errors:#?}"),
//! };
//! ```
//!
//! # Example using HTTPS
//! ```no_run
//! let result = ClientBuilder::unix_http("localhost")
//!                 .enable_https()
//!                 .set_authentication("admin".to_owned(), "pass".to_owned())
//!                 .build_async()
//!                 .run(&["show clock", "show aliases"], ResultFormat::Json)
//!                 .await?;
//! match result {
//!     Response::Result(v) => println!("{v:?}"),
//!     Response::Error {
//!         message,
//!         code,
//!         errors,
//!     } => println!("error code: {code}, message: {message}, outputs: {errors:#?}"),
//! };
//! ```

pub(crate) mod client;

use super::protocol;
use async_trait::async_trait;
use thiserror::Error;

pub use super::{
    ClientBuilder, HttpClientBuilder, Response, ResultFormat, UdsClientBuilder, UseHttp, UseHttps,
};
pub use client::{HttpConnector, HttpsConnector};

/// The errors returned by the library.
#[derive(Debug, Error)]
pub enum Error {
    /// Protocol error (incorrect encoding received from server, etc.)
    #[error("protocol error: {0}")]
    ProtocolError(#[from] protocol::Error),
    /// I/O and other communication errors.
    #[error("communication error: {0}")]
    ClientError(#[from] client::Error),
}

#[async_trait]
/// Commands runner. Clients implement this trait.
pub trait Runner {
    /// Runs commands via eAPI and returns the results.
    ///
    /// # Arguments:
    /// * `commands` is a list of commands to run (they shouldn't end with new line).
    /// * `format` specifies if the results of the commands is plain text or JSON.
    ///
    /// The commands are executed in order and the execution stops at the first command that
    /// results in an error.
    async fn run<S>(self, cmds: &[S], format: ResultFormat) -> Result<Response, Error>
    where
        S: AsRef<str> + Send + Sync;
}

#[async_trait]
impl<T: client::Requester + Send> Runner for T {
    async fn run<S>(self, cmds: &[S], format: ResultFormat) -> Result<Response, Error>
    where
        S: AsRef<str> + Send + Sync,
    {
        let request = protocol::make_run_request(cmds, format);
        let response = self.do_request(request).await?;
        protocol::parse_response(&response).map_err(|e| e.into())
    }
}

impl ClientBuilder<UdsClientBuilder> {
    /// Builds an async Unix domain sockets client.
    pub async fn build_async(self) -> Result<client::UdsClient, Error> {
        let socket_name = self
            .0
            .socket_name
            .unwrap_or_else(|| protocol::make_socket_name(super::SYSNAME));
        client::UdsClient::connect(self.0.sysname, socket_name)
            .await
            .map_err(|e| e.into())
    }
}

impl ClientBuilder<HttpClientBuilder<UseHttp>> {
    /// Builds an async HTTP client.
    pub fn build_async(self) -> client::HttpClient<HttpConnector> {
        client::HttpClient::new_http(self.0.hostname, self.0.auth, self.0.timeout)
    }
}

impl ClientBuilder<HttpClientBuilder<UseHttps>> {
    /// Builds an async HTTPS client.
    pub fn build_async(self) -> client::HttpClient<HttpsConnector> {
        client::HttpClient::new_https(
            self.0.hostname,
            self.0.auth,
            self.0.timeout,
            self.0.https.insecure,
        )
    }
}

/// Runs commands via eAPI and returns the results.
///
/// # Arguments:
/// * `sysname` argument should be left as `None` for running commands on production devices.
/// * `commands` is a list of commands to run (they shouldn't end with new line).
/// * `format` specifies if the results of the commands is plain text or JSON.
///
/// The commands are executed in order and the execution stops at the first command that
/// results in an error.
#[deprecated(since = "0.2.0", note = "please use the `ClientBuilder`")]
pub async fn eapi_run<T: AsRef<str> + Send + Sync>(
    sysname: Option<&str>,
    commands: &[T],
    format: ResultFormat,
) -> Result<Response, Error> {
    let mut builder = ClientBuilder::unix_socket();
    if let Some(sysname) = sysname {
        builder = builder.set_sysname(sysname.to_owned());
    }
    builder.build_async().await?.run(commands, format).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    async fn run_http_server(
        response: &str,
    ) -> (
        u16,
        tokio::sync::oneshot::Sender<()>,
        tokio::sync::mpsc::Receiver<(Vec<u8>, Vec<u8>)>,
    ) {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        let (tx_shut, rx_shut) = tokio::sync::oneshot::channel::<()>();

        let addr = ([127, 0, 0, 1], 0).into();
        let incoming = hyper::server::conn::AddrIncoming::bind(&addr).unwrap();
        let port = incoming.local_addr().port();
        let response = response.to_string();

        tokio::spawn(async move {
            let make_service = hyper::service::make_service_fn(move |_conn| {
                let response = response.clone();
                let sender = sender.clone();
                async move {
                    Ok::<_, Infallible>(hyper::service::service_fn(
                        move |req: hyper::Request<hyper::Body>| {
                            let auth = req
                                .headers()
                                .get("Authorization")
                                .unwrap_or(&hyper::header::HeaderValue::from_static(""))
                                .as_bytes()
                                .to_vec();
                            let response = response.clone();
                            let sender = sender.clone();
                            async move {
                                let body = hyper::body::to_bytes(req.into_body()).await?.to_vec();
                                sender.send((auth, body)).await.unwrap();
                                Ok::<_, hyper::Error>(hyper::Response::new(hyper::Body::from(
                                    response,
                                )))
                            }
                        },
                    ))
                }
            });

            let server = hyper::server::Server::builder(incoming)
                .serve(make_service)
                .with_graceful_shutdown(async {
                    rx_shut.await.ok();
                });
            server.await.unwrap();
        });

        (port, tx_shut, receiver)
    }

    #[tokio::test]
    async fn test_uds_ok() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tmp_dir = tempfile::tempdir()?;
        let socket_name = tmp_dir
            .path()
            .join(crate::SYSNAME)
            .to_str()
            .ok_or("can't convert path to string")?
            .to_string();

        let response = r#"{
            "jsonrpc": "2.0",
            "result": ["test1", "test2", {"a": "b"}],
            "id": "1"
        }"#;

        let sname = socket_name.clone();
        let handle = tokio::task::spawn_blocking(move || {
            let (ready, handle) = crate::tests::run_uds_server(sname, crate::SYSNAME, response);
            ready.wait();
            handle
        })
        .await?;
        let result = ClientBuilder::unix_socket()
            .set_sysname(crate::SYSNAME.to_owned())
            .set_socket_name(socket_name)
            .build_async()
            .await?
            .run(&["show run", "show int", "show clock"], ResultFormat::Json)
            .await?;
        let request = tokio::task::spawn_blocking(|| match handle.join() {
            Ok(r) => r,
            Err(e) => std::panic::resume_unwind(e),
        })
        .await??;
        let expected = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "runCmds",
            "params": {
                "version": "latest",
                "cmds": ["show run", "show int", "show clock"],
                "format": "json",
            },
            "id": "1"
        })
        .to_string();
        assert_eq!(request, expected.as_bytes());
        assert_eq!(
            result,
            Response::Result(vec![
                "\"test1\"".to_string(),
                "\"test2\"".to_string(),
                "{\"a\":\"b\"}".to_string()
            ])
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_uds_error() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tmp_dir = tempfile::tempdir()?;
        let socket_name = tmp_dir
            .path()
            .join(crate::SYSNAME)
            .to_str()
            .ok_or("can't convert path to string")?
            .to_string();

        let response = r#"{
            "jsonrpc": "2.0",
            "error": {
                "message": "error message",
                "code": 3,
                "data": ["a", "b"]
            },
            "id": "1"
        }"#;
        let sname = socket_name.clone();
        let handle = tokio::task::spawn_blocking(move || {
            let (ready, handle) = crate::tests::run_uds_server(sname, crate::SYSNAME, response);
            ready.wait();
            handle
        })
        .await?;
        let result = ClientBuilder::unix_socket()
            .set_sysname(crate::SYSNAME.to_owned())
            .set_socket_name(socket_name)
            .build_async()
            .await?
            .run(&["show run", "show int", "show clock"], ResultFormat::Json)
            .await?;
        let request = tokio::task::spawn_blocking(|| match handle.join() {
            Ok(r) => r,
            Err(e) => std::panic::resume_unwind(e),
        })
        .await??;
        let expected = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "runCmds",
            "params": {
                "version": "latest",
                "cmds": ["show run", "show int", "show clock"],
                "format": "json",
            },
            "id": "1"
        })
        .to_string();
        assert_eq!(request, expected.as_bytes());
        assert_eq!(
            result,
            Response::Error {
                message: "error message".to_string(),
                code: 3,
                errors: vec!["\"a\"".to_string(), "\"b\"".to_string()]
            }
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_http_ok() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let response = r#"{
            "jsonrpc": "2.0",
            "result": ["test1", "test2", {"a": "b"}],
            "id": "1"
        }"#;
        let (port, shutdown, mut receiver) = run_http_server(response).await;
        let result = ClientBuilder::http("localhost:".to_owned() + &port.to_string())
            .set_authentication("admin".to_owned(), "pass".to_owned())
            .build_async()
            .run(&["show run", "show int", "show clock"], ResultFormat::Json)
            .await?;
        let request = receiver.recv().await.unwrap();
        let expected = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "runCmds",
            "params": {
                "version": "latest",
                "cmds": ["show run", "show int", "show clock"],
                "format": "json",
            },
            "id": "1"
        })
        .to_string();
        assert_eq!(request.0, "Basic YWRtaW46cGFzcw==".as_bytes());
        assert_eq!(request.1, expected.as_bytes());
        assert_eq!(
            result,
            Response::Result(vec![
                "\"test1\"".to_string(),
                "\"test2\"".to_string(),
                "{\"a\":\"b\"}".to_string()
            ])
        );

        let _ = shutdown.send(());

        Ok(())
    }

    #[tokio::test]
    async fn test_http_error() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let response = r#"{
            "jsonrpc": "2.0",
            "error": {
                "message": "error message",
                "code": 3,
                "data": ["a", "b"]
            },
            "id": "1"
        }"#;
        let (port, shutdown, mut receiver) = run_http_server(response).await;
        let result = ClientBuilder::http("localhost:".to_owned() + &port.to_string())
            .set_authentication("admin".to_owned(), "pass".to_owned())
            .build_async()
            .run(&["show run", "show int", "show clock"], ResultFormat::Json)
            .await?;
        let request = receiver.recv().await.unwrap();
        let expected = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "runCmds",
            "params": {
                "version": "latest",
                "cmds": ["show run", "show int", "show clock"],
                "format": "json",
            },
            "id": "1"
        })
        .to_string();
        assert_eq!(request.0, "Basic YWRtaW46cGFzcw==".as_bytes());
        assert_eq!(request.1, expected.as_bytes());
        assert_eq!(
            result,
            Response::Error {
                message: "error message".to_string(),
                code: 3,
                errors: vec!["\"a\"".to_string(), "\"b\"".to_string()]
            }
        );

        let _ = shutdown.send(());

        Ok(())
    }
}
