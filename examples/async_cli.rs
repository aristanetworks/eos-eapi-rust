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

use eos_eapi::*;

static USAGE: &str = "usage: cli [-t|-j] cmd1 [cmd2...]";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut format = ResultFormat::default();

    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() > 0 && args[0].starts_with('-') {
        if args[0] == "-t" {
            format = ResultFormat::Text;
        } else if args[0] == "-j" {
            format = ResultFormat::Json;
        } else {
            return Err(USAGE.into());
        }
        args.remove(0);
    }
    if args.len() < 1 {
        return Err(USAGE.into());
    }

    let result = async_api::eapi_run(None, &args, format).await?;
    match result {
        Response::Result(v) => println!("{v:?}"),
        Response::Error {
            message,
            code,
            errors,
        } => println!("error code: {code}, message: {message}, outputs: {errors:#?}"),
    };

    Ok(())
}
