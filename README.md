# EOS eAPI

This crate allows execution of CLI commands on [Arista](https://www.arista.com) EOS switches from
Rust programs.
It supports both blocking and async APIs.

---
```toml
[dependencies]
eos-eapi = "0.2"
```

# Example

API usage example via Unix domain sockets:
```
let result = ClientBuilder::unix_socket()
                .build_blocking()?
                .run(&["show clock", "show aliases"], ResultFormat::Json)?;
match result {
    Response::Result(v) => println!("{v:?}"),
    Response::Error {
        message,
        code,
        errors,
    } => println!("error code: {code}, message: {message}, outputs: {errors:#?}"),
};
```

API usage example via HTTP:
```
let result = ClientBuilder::http("localhost")
                .set_authentication("admin".to_owned(), "pass".to_owned())
                .build_blocking()
                .run(&["show clock", "show aliases"], ResultFormat::Json)?;
match result {
    Response::Result(v) => println!("{v:?}"),
    Response::Error {
        message,
        code,
        errors,
    } => println!("error code: {code}, message: {message}, outputs: {errors:#?}"),
};
```

# Switch configuration

If you're using HTTPS to connect to eAPI, depending on your EOS version, the default HTTPS server
configuration might have outdated settings and the client won't be able to connect.

You need to create a set of keys and certificate:
```
security pki key generate rsa 4096 capikey.pem
security pki certificate generate self-signed capi.pem key capikey.pem validity 3650 parameters common-name HOSTNAME subject-alternative-name dns HOSTNAME
```

And then create an SSL profile with them:
```
config
management security
   ssl profile p01
      tls versions 1.2
      cipher-list ECDHE-RSA-AES256-GCM-SHA384
      certificate capi.pem key capikey.pem
      trust certificate ARISTA_SIGNING_CA.crt
      trust certificate ARISTA_ROOT_CA.crt
```

Apply the profile:
```
management http-server
   protocol http
   protocol https ssl profile p01
```

