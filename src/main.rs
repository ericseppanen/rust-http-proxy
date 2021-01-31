use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use config::Config;

mod config;

// The number of bytes we can read at once while proxying.
const BUFFER_SIZE: usize = 2048;


/// Ferry bytes between two streams.
//
// This does the heavy lifting of the proxy service, so it should
// be implemented as efficiently as possible.
async fn proxy(mut socket1: TcpStream, mut socket2: TcpStream, _config: &Config) -> io::Result<()> {
    // As this is an async fn, these buffers will actually be part of the future's
    // state, so they are allocated on the heap.
    // TODO: wasteful to zero-initialize these. Look into reusing these via a pool?
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

// Note: while this returns a Result, if this is spawned directly into the
// executor, meaning that its result will never be seen. If its result is
// interesting, perhaps a logging wrapper should be used.
async fn process_socket(client_socket: TcpStream, config: &Config) -> io::Result<()> {
    println!("got new socket");

    // make a new TCP connection to the target
    // TODO: cache DNS results
    let target_socket = TcpStream::connect((config.target_host.as_str(), config.target_port)).await.map_err(|e| {
        println!("error connecting to target: {:?}", e);
        e
    })?;

    proxy(client_socket, target_socket, config).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let config = Config::read_config_file();

    println!("listening on {}:{}", config.local_addr, config.local_port);
    let listener = TcpListener::bind((config.local_addr.as_str(), config.local_port)).await?;

    loop {
        let (socket, _socket_addr) = listener.accept().await?;
        // TODO: implement a limit on concurrent connections.
        // TODO: log socket_addr?
        tokio::spawn(process_socket(socket, &config));
    }
}
