# EOS CAPI

This crate allows execution of CLI commands on [Arista](https://www.arista.com) EOS switches.

---
```toml
[dependencies]
eos_capi = "0.1"
```

# Example
```
let result = eapi_run(None, &["show clock", "show aliases"], ResultFormat::Json);
match result {
    Response::Result(v) => println!("{v:?}"),
    Response::Error {
        message,
        code,
        errors,
    } => println!("error code: {code}, message: {message}, outputs: {errors:#?}"),
};

