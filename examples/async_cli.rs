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
