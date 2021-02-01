use config::Config;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

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
async fn proxy(mut socket1: TcpStream, mut socket2: TcpStream, _config: &Config) -> io::Result<()> {
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
                    println!("EOF on socket1");
                    return Ok(())
                }
                println!("got {} bytes on socket1", n);
                socket2.write(&buffer1[..n]).await?;
            }
            n = socket2.read(&mut buffer2) => {
                let n = n?;
                if n == 0 {
                    println!("EOF on socket2");
                    return Ok(())
                }
                println!("got {} bytes on socket2", n);
                socket1.write(&buffer2[..n]).await?;
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
async fn process_socket(mut client_socket: TcpStream, config: &Config) -> io::Result<()> {
    println!("got new socket");

    // Read the HTTP request from the network.
    let http_request = get_http_request(&mut client_socket).await?;
    // Parse the HTTP request to get the target host+port.
    let target_host =
        parse_http_connect(&http_request).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Check whether target host is on our allow list.
    if !config.is_server_allowed(&target_host) {
        println!("deny access to server {:?}", target_host);
        let response = "HTTP/1.0 403 Forbidden\r\n\r\n";
        client_socket.write(response.as_bytes()).await?;
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
    client_socket.write(response.as_bytes()).await?;

    println!("start proxy now");

    proxy(client_socket, target_socket, config).await?;

    Ok(())
}

// Get an http request from the network
// Parse the request, and return the host string if it's a CONNECT.
async fn get_http_request(socket: &mut TcpStream) -> io::Result<Vec<u8>> {
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
    if let Err(e) = parse_result {
        println!("{:#?}", e);
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

#[tokio::main]
async fn main() -> io::Result<()> {
    let config = Config::read_config_file();

    println!("listening on {}:{}", config.local_addr, config.local_port);
    let listener = TcpListener::bind((config.local_addr, config.local_port)).await?;

    loop {
        let (socket, _socket_addr) = listener.accept().await?;
        // TODO: implement a limit on concurrent connections.
        // TODO: log socket_addr?
        tokio::spawn(process_socket(socket, &config));
    }
}
