use thiserror::Error;

#[cfg(feature = "async")]
pub mod async_api;
mod client;
mod protocol;

static SYSNAME: &str = "ar";

#[derive(Debug, Error)]
pub enum Error {
    #[error("protocol error: {0}")]
    ProtocolError(#[from] protocol::Error),
    #[error("communication error: {0}")]
    ClientError(#[from] client::Error),
}

#[derive(Default)]
pub enum ResultFormat {
    #[default]
    Json,
    Text,
}

#[derive(Debug, PartialEq)]
pub enum Response {
    Error {
        message: String,
        code: i64,
        errors: Vec<String>,
    },
    Result(Vec<String>),
}

pub fn eapi_run<T: AsRef<str>>(
    sysname: Option<&str>,
    cmds: &[T],
    format: ResultFormat,
) -> Result<Response, Error> {
    let sysname = sysname.unwrap_or(SYSNAME);
    let mut conn = client::Client::connect(sysname)?;
    let request = protocol::make_run_request(cmds, format);
    let response = conn.do_request(request)?;
    protocol::parse_request(&response).map_err(|e| e.into())
}
