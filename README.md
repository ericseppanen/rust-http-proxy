# A basic Rust http proxy server

This is a server that implements the HTTP CONNECT protocol for proxying to a remote service. Major features:

- Supports an arbitrary number of concurrent connections on a small number of threads.
- Configurable list of remote servers that we're allowed to proxy to.
- Configurable for TLS or plaintext client connections.

### Running the server

You can build and run using `cargo` as expected:

```
cargo run --release
```

The server configuration is in TOML format.  The file `http_proxy_config.toml` will be loaded from the current working directory when the server is started.

If you want to use TLS for the connection from the client, you must set the following values in the config file:

```
use_tls = true
cert_chain = "server_chain.pem"
private_key = "server_priv_key.pem"
```

Example of connecting to the proxy server with `curl` when not using TLS:
```
https_proxy=http://proxy_server:8080/ curl https://www.google.com
```

Example of connecting to the proxy server with `curl`, when using TLS:

```
https_proxy=https://proxy_server:8080/ curl --proxy-cacert my_root_cert.pem https://www.google.com
```
or if you're really reckless:
```
https_proxy=https://proxy_server:8080/ curl --proxy-insecure https://www.google.com
```

### Bugs, unfinished work, future improvements

- No client authentication is supported.
- Only RSA keys are supported, and only in PKCS8 PEM format.
- Only PEM format certificate chains are supported. All certificates must be concatenated into a single file.
- The logging in the server doesn't have any configurability. It only logs to stdout, and there's no way to tell which message is from which client connection.
- Error handling is not very expressive; io::ErrorKind::Other is over-used.
- DNS results should be cached.
- Statistics about connection counts, errors, latencies, etc. would be useful.
- The number of threads should be configurable.
- There should be a configurable limit on the number of concurrent connections allowed.
- There should be a timeout on clients: if they can't complete their data transfer in a certain amount of time, the socket should be closed.
- There aren't any tests. Since most of the functionality has to do with network I/O, unit tests will require mock network connections that transmit different kinds of data (e.g. TLS handshakes).
