use eos_capi::*;

static USAGE: &str = "usage: cli [-t|-j] cmd1 [cmd2...]";

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let result = eapi_run(None, &args, format)?;
    println!("{:?}", result);

    Ok(())
}
