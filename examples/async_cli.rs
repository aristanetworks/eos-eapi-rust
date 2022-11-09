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

use clap::{Parser, Subcommand, ValueEnum};
use eos_eapi::async_api::*;

#[derive(Subcommand)]
enum Commands {
    /// Connect via unix domain socket
    Uds {
        #[arg(required = true)]
        commands: Vec<String>,
    },
    /// Connect via HTTP
    Http {
        hostname: String,
        #[arg(short, long, requires = "password")]
        user: Option<String>,
        #[arg(short, long, requires = "user")]
        password: Option<String>,
        #[arg(required = true)]
        commands: Vec<String>,
    },
    /// Connect via HTTPS
    Https {
        /// Skip certification validation
        #[arg(short = 'k', long)]
        insecure: bool,
        hostname: String,
        #[arg(short, long, requires = "password")]
        user: Option<String>,
        #[arg(short, long, requires = "user")]
        password: Option<String>,
        #[arg(required = true)]
        commands: Vec<String>,
    },
}

#[derive(ValueEnum, Clone)]
enum Format {
    Json,
    Text,
}

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    subcmd: Commands,
    /// Results format
    #[arg(short, long, default_value = "text")]
    format: Format,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args = Args::parse();

    let format = match args.format {
        Format::Json => ResultFormat::Json,
        Format::Text => ResultFormat::Text,
    };

    let result = match args.subcmd {
        Commands::Uds { commands } => {
            ClientBuilder::unix_socket()
                .build_async()
                .await?
                .run(&commands, format)
                .await?
        }
        Commands::Http {
            hostname,
            user,
            password,
            commands,
        } => {
            if let (Some(user), Some(password)) = (user, password) {
                ClientBuilder::http(hostname).set_authentication(user, password)
            } else {
                ClientBuilder::http(hostname)
            }
            .build_async()
            .run(&commands, format)
            .await?
        }
        Commands::Https {
            insecure,
            hostname,
            user,
            password,
            commands,
        } => {
            if let (Some(user), Some(password)) = (user, password) {
                ClientBuilder::http(hostname).set_authentication(user, password)
            } else {
                ClientBuilder::http(hostname)
            }
            .enable_https()
            .set_insecure(insecure)
            .build_async()
            .run(&commands, format)
            .await?
        }
    };

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
