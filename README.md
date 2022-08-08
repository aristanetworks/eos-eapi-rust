# EOS eAPI

This crate allows execution of CLI commands on [Arista](https://www.arista.com) EOS switches from
Rust programs.

---
```toml
[dependencies]
eos_eapi = "0.1"
```

# Configuring EOS Command API on the switch

To enable EOS Command API from configuration mode, configure proper protocol under management api, and then verify:
```
    Switch# configure terminal
    Switch(config)# management api http-commands
    Switch(config-mgmt-api-http-cmds)# protocol ?
      http         Configure HTTP server options
      https        Configure HTTPS server options
      unix-socket  Configure Unix Domain Socket
    Switch(config-mgmt-api-http-cmds)# protocol http
    Switch(config-mgmt-api-http-cmds)# end

    Switch# show management api http-commands
    Enabled:            Yes
    HTTPS server:       running, set to use port 443
    HTTP server:        running, set to use port 80
    Local HTTP server:  shutdown, no authentication, set to use port 8080
    Unix Socket server: shutdown, no authentication
    ...
```

# Example

API usage example:
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

