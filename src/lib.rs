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

//! This crate allows execution of CLI commands on [Arista](https://www.arista.com) EOS switches.
//!
//! # Features
//! * `blocking` (default) blocking API.
//! * `async` adds async (via [tokio](https://tokio.rs) runtime) support.
//!
//! # Example using unix sockets
//! The UDS client is one shot, and `run` consumes the client.
//! ```no_run
//! let result = ClientBuilder::unix_socket()
//!                 .build_blocking()?
//!                 .run(&["show clock", "show aliases"], ResultFormat::Json)?;
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
//!                 .build_blocking()
//!                 .run(&["show clock", "show aliases"], ResultFormat::Json)?;
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
//!                 .build_blocking()
//!                 .run(&["show clock", "show aliases"], ResultFormat::Json)?;
//! match result {
//!     Response::Result(v) => println!("{v:?}"),
//!     Response::Error {
//!         message,
//!         code,
//!         errors,
//!     } => println!("error code: {code}, message: {message}, outputs: {errors:#?}"),
//! };
//! ```

use thiserror::Error;

#[cfg(feature = "async")]
pub mod async_api;
mod client;
mod protocol;

static SYSNAME: &str = "ar";

#[cfg(feature = "blocking")]
pub use client::{HttpClient, UdsClient};

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

/// Format of the commands output.
#[derive(Default)]
pub enum ResultFormat {
    /// JSONified output
    #[default]
    Json,
    /// Plain text (same as in the CLI output)
    Text,
}

/// Commands execution response.
#[derive(Debug, PartialEq, Eq)]
pub enum Response {
    /// Error response
    Error {
        /// Error message
        message: String,
        /// Error code
        code: i64,
        /// For each command, either the output or the error associated with it.
        /// The execution stops at the first encountered error, so length of `errors` might be
        /// smaller than number of commands.
        errors: Vec<String>,
    },
    Result(Vec<String>),
}

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
    fn run<S: AsRef<str>>(self, cmds: &[S], format: ResultFormat) -> Result<Response, Error>;
}

impl<T: client::Requester> Runner for T {
    fn run<S: AsRef<str>>(self, cmds: &[S], format: ResultFormat) -> Result<Response, Error> {
        let request = protocol::make_run_request(cmds, format);
        let response = self.do_request(request)?;
        protocol::parse_response(&response).map_err(|e| e.into())
    }
}

#[doc(hidden)]
pub struct UdsClientBuilder {
    sysname: String,
    socket_name: Option<String>,
}

impl ClientBuilder<UdsClientBuilder> {
    /// Sets the system name (usually not required).
    pub fn set_sysname(mut self, sysname: String) -> Self {
        self.0.sysname = sysname;
        self
    }

    /// Sets the Unix socket name (usually not required).
    pub fn set_socket_name(mut self, socket_name: String) -> Self {
        self.0.socket_name = Some(socket_name);
        self
    }

    #[cfg(feature = "blocking")]
    /// Builds a blocking Unix domain sockets client.
    pub fn build_blocking(self) -> Result<UdsClient, Error> {
        let socket_name = self
            .0
            .socket_name
            .unwrap_or_else(|| protocol::make_socket_name(SYSNAME));
        UdsClient::connect(self.0.sysname, socket_name).map_err(|e| e.into())
    }
}

#[doc(hidden)]
pub struct UseHttp(());
#[doc(hidden)]
pub struct UseHttps {
    insecure: bool,
}
#[doc(hidden)]
pub struct HttpClientBuilder<T> {
    hostname: String,
    auth: Option<(String, String)>,
    timeout: std::time::Duration,
    https: T,
}

impl<T> ClientBuilder<HttpClientBuilder<T>> {
    /// Sets the credentials.
    pub fn set_authentication(mut self, username: String, password: String) -> Self {
        self.0.auth = Some((username, password));
        self
    }

    /// Sets the timeout.
    /// The timeout includes resolving the host, sending the request and executing the commands.
    pub fn set_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.0.timeout = timeout;
        self
    }

    /// Use HTTPS.
    pub fn enable_https(self) -> ClientBuilder<HttpClientBuilder<UseHttps>> {
        ClientBuilder(HttpClientBuilder {
            hostname: self.0.hostname,
            auth: self.0.auth,
            timeout: self.0.timeout,
            https: UseHttps { insecure: false },
        })
    }
}

impl ClientBuilder<HttpClientBuilder<UseHttp>> {
    #[cfg(feature = "blocking")]
    /// Builds a blocking HTTP client.
    pub fn build_blocking(self) -> HttpClient {
        HttpClient::new_http(self.0.hostname, self.0.auth, self.0.timeout)
    }
}

impl ClientBuilder<HttpClientBuilder<UseHttps>> {
    /// Skips server certificate validation (useful in test scenarios when you have self-signed
    /// certificates).
    pub fn set_insecure(mut self, value: bool) -> Self {
        self.0.https.insecure = value;
        self
    }

    #[cfg(feature = "blocking")]
    /// Builds a blocking HTTPS client.
    pub fn build_blocking(self) -> HttpClient {
        HttpClient::new_https(
            self.0.hostname,
            self.0.auth,
            self.0.timeout,
            self.0.https.insecure,
        )
    }
}

/// Builds a client to connect to eAPI.
pub struct ClientBuilder<T>(T);

impl ClientBuilder<()> {
    /// Build a client using Unix domain sockets to connect to eAPI.
    pub fn unix_socket() -> ClientBuilder<UdsClientBuilder> {
        ClientBuilder(UdsClientBuilder {
            sysname: SYSNAME.to_string(),
            socket_name: None,
        })
    }

    /// Build a client using HTTP(S) to connect to eAPI.
    pub fn http(hostname: String) -> ClientBuilder<HttpClientBuilder<UseHttp>> {
        ClientBuilder(HttpClientBuilder {
            hostname,
            auth: None,
            timeout: std::time::Duration::from_secs(30),
            https: UseHttp(()),
        })
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
#[cfg(feature = "blocking")]
pub fn eapi_run<T: AsRef<str>>(
    sysname: Option<&str>,
    commands: &[T],
    format: ResultFormat,
) -> Result<Response, Error> {
    let mut builder = ClientBuilder::unix_socket();
    if let Some(sysname) = sysname {
        builder = builder.set_sysname(sysname.to_owned());
    }
    builder.build_blocking()?.run(commands, format)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::sys;
    use std::convert::Infallible;
    use std::io::IoSliceMut;
    use std::os::unix::io::RawFd;

    fn rcv_string(socket: RawFd) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let mut len = [0; 4];
        sys::socket::recv(socket, &mut len, sys::socket::MsgFlags::MSG_WAITALL)?;
        let len = i32::from_le_bytes(len) as usize;

        let mut buf = vec![0; len];
        sys::socket::recv(socket, &mut buf, sys::socket::MsgFlags::MSG_WAITALL)?;

        Ok(buf)
    }

    fn rcv_fd<const N: usize>(
        socket: RawFd,
    ) -> Result<Vec<RawFd>, Box<dyn std::error::Error + Send + Sync>> {
        let mut buf = [0];
        let mut iov = [IoSliceMut::new(&mut buf)];
        let mut cmsg_buf: Vec<u8> = nix::cmsg_space!([RawFd; N]);

        let mut result = Vec::with_capacity(N);
        loop {
            let rcv = sys::socket::recvmsg::<()>(
                socket,
                &mut iov,
                Some(&mut cmsg_buf),
                sys::socket::MsgFlags::empty(),
            )?;

            for cmsg in rcv.cmsgs() {
                if let sys::socket::ControlMessageOwned::ScmRights(fds) = cmsg {
                    result.extend(fds)
                } else {
                    return Err("didn't receive SCM_RIGHTS message".into());
                }
            }
            if result.len() == N {
                break Ok(result);
            }
        }
    }

    pub fn run_uds_server<T: AsRef<str>>(
        socket_name: T,
        sysname: &str,
        response: &str,
    ) -> (
        std::sync::Arc<std::sync::Barrier>,
        std::thread::JoinHandle<Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>>,
    ) {
        let socket_name = socket_name.as_ref().to_string();
        let sysname = sysname.to_string();
        let response = response.to_string();

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let ready = barrier.clone();
        let handle = std::thread::spawn(move || {
            let socket = sys::socket::socket(
                sys::socket::AddressFamily::Unix,
                sys::socket::SockType::Stream,
                sys::socket::SockFlag::empty(),
                None,
            )?;

            sys::socket::bind(socket, &sys::socket::UnixAddr::new(socket_name.as_str())?)?;
            sys::socket::listen(socket, 1)?;
            ready.wait();

            let stream = sys::socket::accept(socket)?;

            let signal = rcv_fd::<1>(stream)?[0];

            if rcv_string(stream)? != protocol::make_args(sysname) {
                return Err("received invalid args".into());
            }

            if rcv_string(stream)? != protocol::make_env()? {
                return Err("received invalid env".into());
            }

            if rcv_string(stream)? != "0".as_bytes() {
                return Err("received invalid uid".into());
            }

            if rcv_string(stream)? != "0".as_bytes() {
                return Err("received invalid gid".into());
            }

            if rcv_string(stream)? != "".as_bytes() {
                return Err("received invalid terminal name".into());
            }

            let mut buf = [0];
            sys::socket::recv(stream, &mut buf, sys::socket::MsgFlags::empty())?;
            if buf[0] != b'c' {
                return Err("received invalid mode".into());
            }

            let sockets = rcv_fd::<3>(stream)?;
            let resp_socket = sockets[0];
            let req_socket = sockets[1];
            let stats = sockets[2];

            nix::unistd::close(stream)?;

            let request = rcv_string(req_socket)?;
            sys::socket::send(
                resp_socket,
                response.as_bytes(),
                sys::socket::MsgFlags::empty(),
            )?;

            nix::unistd::close(signal)?;
            nix::unistd::close(req_socket)?;
            nix::unistd::close(resp_socket)?;
            nix::unistd::close(stats)?;

            nix::unistd::close(socket)?;

            Ok(request)
        });

        (barrier, handle)
    }

    fn run_http_server(
        response: &str,
    ) -> (
        u16,
        tokio::sync::oneshot::Sender<()>,
        tokio::sync::mpsc::Receiver<(Vec<u8>, Vec<u8>)>,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        let (tx_shut, rx_shut) = tokio::sync::oneshot::channel::<()>();

        let addr = ([127, 0, 0, 1], 0).into();
        let incoming = {
            let _guard = rt.enter();
            hyper::server::conn::AddrIncoming::bind(&addr).unwrap()
        };
        let port = incoming.local_addr().port();
        let response = response.to_string();

        std::thread::spawn(move || {
            rt.block_on(async move {
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
                                    let body =
                                        hyper::body::to_bytes(req.into_body()).await?.to_vec();
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
            })
        });

        (port, tx_shut, receiver)
    }

    #[test]
    fn test_uds_ok() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tmp_dir = tempfile::tempdir()?;
        let socket_name = tmp_dir
            .path()
            .join(SYSNAME)
            .to_str()
            .ok_or("can't convert path to string")?
            .to_string();

        let response = r#"{
            "jsonrpc": "2.0",
            "result": ["test1", "test2", {"a": "b"}],
            "id": "1"
        }"#;
        let (ready, handle) = run_uds_server(&socket_name, SYSNAME, response);
        ready.wait();
        let result = ClientBuilder::unix_socket()
            .set_sysname(SYSNAME.to_owned())
            .set_socket_name(socket_name)
            .build_blocking()?
            .run(&["show run", "show int", "show clock"], ResultFormat::Json)?;
        let request = match handle.join() {
            Ok(r) => r?,
            Err(e) => std::panic::resume_unwind(e),
        };
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

    #[test]
    fn test_uds_error() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tmp_dir = tempfile::tempdir()?;
        let socket_name = tmp_dir
            .path()
            .join(SYSNAME)
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
        let (ready, handle) = run_uds_server(&socket_name, SYSNAME, response);
        ready.wait();
        let result = ClientBuilder::unix_socket()
            .set_sysname(SYSNAME.to_owned())
            .set_socket_name(socket_name)
            .build_blocking()?
            .run(&["show run", "show int", "show clock"], ResultFormat::Json)?;
        let request = match handle.join() {
            Ok(r) => r?,
            Err(e) => std::panic::resume_unwind(e),
        };
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

    #[test]
    fn test_http_ok() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let response = r#"{
            "jsonrpc": "2.0",
            "result": ["test1", "test2", {"a": "b"}],
            "id": "1"
        }"#;
        let (port, shutdown, mut receiver) = run_http_server(response);
        let result = ClientBuilder::http("localhost:".to_owned() + &port.to_string())
            .set_authentication("admin".to_owned(), "pass".to_owned())
            .build_blocking()
            .run(&["show run", "show int", "show clock"], ResultFormat::Json)?;
        let request = receiver.blocking_recv().unwrap();
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

    #[test]
    fn test_http_error() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let response = r#"{
            "jsonrpc": "2.0",
            "error": {
                "message": "error message",
                "code": 3,
                "data": ["a", "b"]
            },
            "id": "1"
        }"#;
        let (port, shutdown, mut receiver) = run_http_server(response);
        let result = ClientBuilder::http("localhost:".to_owned() + &port.to_string())
            .set_authentication("admin".to_owned(), "pass".to_owned())
            .build_blocking()
            .run(&["show run", "show int", "show clock"], ResultFormat::Json)?;
        let request = receiver.blocking_recv().unwrap();
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
