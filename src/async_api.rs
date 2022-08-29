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

mod client;

use super::{protocol, Response, ResultFormat};
use thiserror::Error;

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

async fn eapi_run_internal<T1: AsRef<str>, T2: AsRef<str>>(
    socket_name: T1,
    sysname: &str,
    cmds: &[T2],
    format: ResultFormat,
) -> Result<Response, Error> {
    let mut conn = client::Client::connect(sysname, socket_name).await?;
    let request = protocol::make_run_request(cmds, format);
    let response = conn.do_request(request).await?;
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
pub async fn eapi_run<T: AsRef<str>>(
    sysname: Option<&str>,
    cmds: &[T],
    format: ResultFormat,
) -> Result<Response, Error> {
    let sysname = sysname.unwrap_or(super::SYSNAME);
    eapi_run_internal(protocol::make_socket_name(sysname), sysname, cmds, format).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_eapi_run_ok() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
            let (ready, handle) = crate::tests::run_server(sname, crate::SYSNAME, response);
            ready.wait();
            handle
        })
        .await?;
        let result = eapi_run_internal(
            &socket_name,
            crate::SYSNAME,
            &["show run", "show int", "show clock"],
            ResultFormat::Json,
        )
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
    async fn test_eapi_run_error() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
            let (ready, handle) = crate::tests::run_server(sname, crate::SYSNAME, response);
            ready.wait();
            handle
        })
        .await?;
        let result = eapi_run_internal(
            &socket_name,
            crate::SYSNAME,
            &["show run", "show int", "show clock"],
            ResultFormat::Json,
        )
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
}
