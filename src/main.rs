use config::Config;
use std::sync::Arc;
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::rustls::{NoClientAuth, ServerConfig};
use tokio_rustls::TlsAcceptor;

mod certs;
mod config;

// The number of bytes we can read at once while proxying.
const BUFFER_SIZE: usize = 2048;

// The maximum size of the HTTP CONNECT request.
const HTTP_BUFFER_SIZE: usize = 2048;

// The number of headers we allow to be attached to the CONNECT request
const MAX_HTTP_HEADERS: usize = 16;

/// Ferry bytes between two TCP streams.
//
// This does the heavy lifting of the proxy service, so it should
// be implemented as efficiently as possible.
async fn proxy<T>(mut socket1: T, mut socket2: TcpStream, _config: &Config) -> io::Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    // As this is an async fn, these buffers will actually be part of the Future's
    // state, so they are allocated on the heap.
    let mut buffer1 = [0u8; BUFFER_SIZE];
    let mut buffer2 = [0u8; BUFFER_SIZE];

    // We can't know who will speak next at any point in time, so we should
    // select! both readers. It's possible for a blocked writer to prevent
    // forward progress, which would imply we should handle the two streams
    // as two separate reader/writer pairs.

    loop {
        tokio::select! {
            n = socket1.read(&mut buffer1) => {
                let n = n?;
                if n == 0 {
                    return Ok(())
                }
                socket2.write_all(&buffer1[..n]).await?;
            }
            n = socket2.read(&mut buffer2) => {
                let n = n?;
                if n == 0 {
                    return Ok(())
                }
                socket1.write_all(&buffer2[..n]).await?;
            }
        }
    }

    // TODO: add byte limits, to prevent misuse?
}

// Handle an incoming client connection.
//
// Note: while this returns a Result, if this is spawned directly into the
// executor, meaning that its result will never be seen. If its result is
// interesting, perhaps a logging wrapper should be used.
async fn process_socket<T>(mut client_socket: T, config: &Config) -> io::Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    // Read the HTTP request from the network.
    let http_request = get_http_request(&mut client_socket).await?;
    // Parse the HTTP request to get the target host+port.
    let target_host =
        parse_http_connect(&http_request).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Check whether target host is on our allow list.
    if !config.is_server_allowed(&target_host) {
        println!("deny access to server {:?}", target_host);
        let response = "HTTP/1.0 403 Forbidden\r\n\r\n";
        client_socket.write_all(response.as_bytes()).await?;
        return Err(io::Error::new(io::ErrorKind::Other, "disallowed host"));
    }

    // make a new TCP connection to the target
    // TODO: cache DNS results
    let target_socket = TcpStream::connect(target_host).await.map_err(|e| {
        println!("error connecting to target: {:?}", e);
        // FIXME: return an HTTP error to the client.
        e
    })?;

    // We're now connected to the target host. Tell the client we're ready.
    let response = "HTTP/1.0 200 Connection Established\r\n\r\n";
    client_socket.write_all(response.as_bytes()).await?;

    println!("remote connection successful, proxy active");

    proxy(client_socket, target_socket, config).await?;

    Ok(())
}

// Get an http request from the network
// Parse the request, and return the host string if it's a CONNECT.
async fn get_http_request<T>(socket: &mut T) -> io::Result<Vec<u8>>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    // It would be nice to be able to not initialize this buffer, since
    // we're just going to overwrite it. Something like read_buf (Rust
    // RFC 2930) would be nice.
    let mut buf: Vec<u8> = vec![0; HTTP_BUFFER_SIZE];
    let mut index = 0usize;
    let mut total_read = 0usize;
    loop {
        let bytes_read = socket.read(&mut buf[index..]).await?;
        if bytes_read == 0 {
            // This will happen either if the socket was closed, or if
            // buf_remain has shrunk to size 0.
            return Err(io::Error::new(io::ErrorKind::Other, "no http request"));
        }
        index += bytes_read;
        total_read += bytes_read;

        // FIXME: what if the client sends trailing bytes?
        if buf[..total_read].ends_with(&b"\r\n\r\n"[..]) {
            buf.truncate(total_read);
            return Ok(buf);
        }
    }
}

// Very simple parsing of an HTTP CONNECT request.
// It either returns the target hostname(+port), or an error.
//
// This ignores everything except for "CONNECT <host> HTTP/1.1"
// There are probably many reasons to be smarter than this:
// - We may want to allow some form of authentication
// - We way want to reject requests that aren't well-formed
fn parse_http_connect(request_buf: &[u8]) -> Result<String, &'static str> {
    let mut headers = [httparse::EMPTY_HEADER; MAX_HTTP_HEADERS];
    let mut request = httparse::Request::new(&mut headers);
    let parse_result = request.parse(request_buf);
    if parse_result.is_err() {
        return Err("failed to parse http request");
    }
    if request.method != Some("CONNECT") {
        return Err("http request not CONNECT");
    }
    let target = match request.path {
        Some(name) => String::from(name),
        None => return Err("http request without target"),
    };

    println!("got CONNECT with target {}", target);
    Ok(target)
}

// Perform the TLS handshake, and then hand off the rest of the communication.
async fn tls_accept(
    acceptor: TlsAcceptor,
    socket: TcpStream,
    config: &'static Config,
) -> io::Result<()> {
    let tls_stream = acceptor.accept(socket).await.map_err(|e| {
        println!("tls stream failed to initialize: {:?}", e);
        e
    })?;
    process_socket(tls_stream, config).await
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let config = Config::read_config_file()?;

    if config.use_tls {
        let (cert_chain, private_key) = config.get_cert_filenames()?;

        // Load the TLS certificate chain and private key
        let tls_certs = certs::load_certs(cert_chain)?;
        let private_key = certs::load_private_key(private_key)?;

        // Set up the TLS server machinery.
        let mut tls_server_config = ServerConfig::new(NoClientAuth::new());
        tls_server_config
            .set_single_cert(tls_certs, private_key)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "tls server config error"))?;
        let tls_server_config = Arc::new(tls_server_config);
        let tls_acceptor = TlsAcceptor::from(tls_server_config);

        // Start listening on the TCP socket
        println!("listening on {}:{}", config.local_addr, config.local_port);
        let listener = TcpListener::bind((config.local_addr, config.local_port)).await?;

        // Accept new connections and spawn their handler into the background.
        loop {
            let (socket, socket_addr) = listener.accept().await?;
            println!("new connection from {}", socket_addr);
            // TODO: implement a limit on concurrent connections.

            tokio::spawn(tls_accept(tls_acceptor.clone(), socket, config));
        }
    } else {
        // This path is used if config.use_tls is false.
        // The client connection will be unencrypted (plaintext TCP).
        // This might be tolerable, if we don't care that an observer can see
        // what remote server is being requested.

        // Start listening on the TCP socket
        println!("listening on {}:{}", config.local_addr, config.local_port);
        let listener = TcpListener::bind((config.local_addr, config.local_port)).await?;

        // Accept new connections and spawn their handler into the background.
        loop {
            let (socket, socket_addr) = listener.accept().await?;
            println!("new connection from {}", socket_addr);

            // TODO: implement a limit on concurrent connections.

            tokio::spawn(process_socket(socket, config));
        }
    }
}
