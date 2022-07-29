use super::{Response, ResultFormat};
use std::os::unix::prelude::*;
use thiserror::Error;

static SERVER_PATH: &str = "/CliServer/";
static ARG_STRING: &[u8] = b"-A\x00-p=15\x00-s=";
static ENV_VARS: [&str; 9] = [
    "LOGNAME",
    "USER",
    "SHELL",
    "HOME",
    "PATH",
    "LANG",
    "TERM",
    "TRACE",
    "TRACEFILE",
];

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    #[error("cannot interpret response: {0}")]
    MalformedResponse(String),
    #[error("cannot parse response: {0}")]
    InvalidJSON(#[from] serde_json::Error),
}

pub fn make_socket_name<T: AsRef<str>>(sysname: T) -> String {
    let mut result = String::from(SERVER_PATH);
    result.push_str(sysname.as_ref());

    result
}

pub fn make_args<T: AsRef<str>>(sysname: T) -> Vec<u8> {
    let sysname = sysname.as_ref();
    let mut result = Vec::with_capacity(sysname.len() + ARG_STRING.len());
    result.extend_from_slice(ARG_STRING);
    result.extend_from_slice(sysname.as_bytes());

    result
}

fn add_var(output: &mut Vec<u8>, name: &str, value: &[u8]) {
    output.extend_from_slice(name.as_bytes());
    output.push(0);
    output.extend_from_slice(value);
    output.push(0);
}

pub fn make_env() -> Result<Vec<u8>, std::io::Error> {
    let mut result = Vec::new();

    for var in ENV_VARS {
        add_var(
            &mut result,
            var,
            std::env::var_os(var).as_ref().map_or(b"", |v| v.as_bytes()),
        );
    }

    add_var(
        &mut result,
        "PWD",
        std::env::current_dir()?.as_os_str().as_bytes(),
    );
    add_var(
        &mut result,
        "UID",
        nix::unistd::geteuid().to_string().as_bytes(),
    );
    add_var(
        &mut result,
        "GID",
        nix::unistd::getegid().to_string().as_bytes(),
    );

    Ok(result)
}

pub fn make_run_request<T: AsRef<str>>(cmds: &[T], format: ResultFormat) -> String {
    let cmds: Vec<&str> = cmds.iter().map(AsRef::as_ref).collect();
    let fmt = match format {
        ResultFormat::Json => "json",
        ResultFormat::Text => "text",
    };

    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "runCmds",
        "params": {
            "version": "latest",
            "cmds": cmds,
            "format": fmt,
        },
        "id": "1"
    })
    .to_string()
}

pub fn parse_request(result: &[u8]) -> Result<Response, Error> {
    let response: serde_json::Value = serde_json::from_slice(result)?;
    if let Some(error) = response.get("error") {
        let message = error
            .get("message")
            .ok_or_else(|| Error::MalformedResponse("error object has no message".to_string()))?
            .as_str()
            .ok_or_else(|| Error::MalformedResponse("error message is not a string".to_string()))?
            .to_string();
        let code = error
            .get("code")
            .ok_or_else(|| Error::MalformedResponse("error object has no code".to_string()))?
            .as_i64()
            .ok_or_else(|| Error::MalformedResponse("error code is not numeric".to_string()))?;
        let data = error
            .get("data")
            .ok_or_else(|| Error::MalformedResponse("error object has no data".to_string()))?
            .as_array()
            .ok_or_else(|| Error::MalformedResponse("error data is not an array".to_string()))?;
        let errors = data
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<_, _>>()?;
        return Ok(Response::Error {
            message,
            code,
            errors,
        });
    }
    if let Some(result) = response.get("result") {
        let result = result
            .as_array()
            .ok_or_else(|| Error::MalformedResponse("result is not an array".to_string()))?;
        let results = result
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<_, _>>()?;
        return Ok(Response::Result(results));
    }

    Err(Error::MalformedResponse(
        "RPC response doesn't contain result or errors".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_socket_name() {
        let name = "testsys";
        let result = make_socket_name(name);
        assert_eq!(result, "/CliServer/testsys");
    }

    #[test]
    fn test_make_args() {
        let name = "testsys";
        let result = make_args(name);
        assert_eq!(result, b"-A\x00-p=15\x00-s=testsys");
    }

    #[test]
    fn test_make_env() -> Result<(), Box<dyn std::error::Error>> {
        let vars = vec![
            "LOGNAME",
            "USER",
            "SHELL",
            "HOME",
            "PATH",
            "LANG",
            "TERM",
            "TRACE",
            "TRACEFILE",
            "PWD",
            "UID",
            "GID",
        ];

        let result = make_env()?;
        for var in vars {
            let i = result
                .windows(var.len())
                .position(|w| w == var.as_bytes())
                .ok_or(format!("can't find var {}", var))?;
            if i > 0 {
                assert_eq!(result[i - 1], 0);
            }
            assert_eq!(result[i + var.len()], 0);
        }
        Ok(())
    }

    #[test]
    fn test_make_run_request() {
        let mut expected = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "runCmds",
            "params": {
                "version": "latest",
                "cmds": ["show run", "alias x y"],
                "format": "json",
            },
            "id": "1"
        })
        .to_string();
        let mut result = make_run_request(&vec!["show run", "alias x y"], ResultFormat::Json);
        assert_eq!(result, expected);

        expected = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "runCmds",
            "params": {
                "version": "latest",
                "cmds": ["show run", "alias x y"],
                "format": "text",
            },
            "id": "1"
        })
        .to_string();
        result = make_run_request(&vec!["show run", "alias x y"], ResultFormat::Text);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_request() -> Result<(), Box<dyn std::error::Error>> {
        let mut input = r#"{
            "jsonrpc": "2.0",
            "result": ["test1", "test2", {"a": "b"}],
            "id": "1"
        }"#;
        let mut result = parse_request(input.as_bytes())?;
        assert_eq!(
            result,
            Response::Result(vec![
                "\"test1\"".to_string(),
                "\"test2\"".to_string(),
                "{\"a\":\"b\"}".to_string()
            ])
        );

        input = r#"{
            "jsonrpc": "2.0",
            "error": {
                "message": "error message",
                "code": 3,
                "data": ["a", "b"]
            },
            "id": "1"
        }"#;
        result = parse_request(input.as_bytes())?;
        assert_eq!(
            result,
            Response::Error {
                message: "error message".to_string(),
                code: 3,
                errors: vec!["\"a\"".to_string(), "\"b\"".to_string()]
            }
        );

        input = r#"{
            "jsonrpc": "2.0",
            "id": "1"
        }"#;
        assert!(matches!(
            parse_request(input.as_bytes()),
            Err(Error::MalformedResponse(_))
        ));

        input = r#"{
            "jsonrpc": "2.0,
            "id": "1"
        }"#;
        assert!(matches!(
            parse_request(input.as_bytes()),
            Err(Error::InvalidJSON(_))
        ));

        Ok(())
    }
}
