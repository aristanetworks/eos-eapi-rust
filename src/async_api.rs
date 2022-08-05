mod client;

use super::{protocol, Response, ResultFormat};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("protocol error: {0}")]
    ProtocolError(#[from] protocol::Error),
    #[error("communication error: {0}")]
    ClientError(#[from] client::Error),
}

pub async fn eapi_run<T: AsRef<str>>(
    sysname: Option<&str>,
    cmds: &[T],
    format: ResultFormat,
) -> Result<Response, Error> {
    let sysname = sysname.unwrap_or(super::SYSNAME);
    let mut conn = client::Client::connect(sysname).await?;
    let request = protocol::make_run_request(cmds, format);
    let response = conn.do_request(request).await?;
    protocol::parse_request(&response).map_err(|e| e.into())
}
