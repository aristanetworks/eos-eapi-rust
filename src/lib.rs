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
//! * `async` adds async (via [tokio](https://tokio.rs) runtime) support.
//!
//! # Example
//! ```
//! let result = eapi_run(None, &["show clock", "show aliases"], ResultFormat::Json);
//! match result {
//!     Response::Result(v) => println!("{v:?}"),
//!     Response::Error {
//!         message,
//!         code,
//!         errors,
//!     } => println!("error code: {code}, message: {message}, outputs: {errors:#?}"),
//! };

use thiserror::Error;

#[cfg(feature = "async")]
pub mod async_api;
mod client;
mod protocol;

static SYSNAME: &str = "ar";

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
#[derive(Debug, PartialEq)]
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

fn eapi_run_internal<T1: AsRef<str>, T2: AsRef<str>>(
    socket_name: T1,
    sysname: &str,
    cmds: &[T2],
    format: ResultFormat,
) -> Result<Response, Error> {
    let socket_name = socket_name.as_ref();
    let mut conn = client::Client::connect(sysname, socket_name)?;
    let request = protocol::make_run_request(cmds, format);
    let response = conn.do_request(request)?;
    protocol::parse_response(&response).map_err(|e| e.into())
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
pub fn eapi_run<T: AsRef<str>>(
    sysname: Option<&str>,
    commands: &[T],
    format: ResultFormat,
) -> Result<Response, Error> {
    let sysname = sysname.unwrap_or(SYSNAME);
    eapi_run_internal(
        protocol::make_socket_name(sysname),
        sysname,
        commands,
        format,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::sys;
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

    pub fn run_server<T: AsRef<str>>(
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

    #[test]
    fn test_eapi_run_ok() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
        let (ready, handle) = run_server(&socket_name, SYSNAME, response);
        ready.wait();
        let result = eapi_run_internal(
            &socket_name,
            SYSNAME,
            &["show run", "show int", "show clock"],
            ResultFormat::Json,
        )?;
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
    fn test_eapi_run_error() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
        let (ready, handle) = run_server(&socket_name, SYSNAME, response);
        ready.wait();
        let result = eapi_run_internal(
            &socket_name,
            SYSNAME,
            &["show run", "show int", "show clock"],
            ResultFormat::Json,
        )?;
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
}
