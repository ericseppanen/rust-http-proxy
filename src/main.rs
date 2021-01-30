use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Ferry bytes between two streams.
//
// This does the heavy lifting of the proxy service, so it should
// be implemented as efficiently as possible.
async fn proxy(mut socket1: TcpStream, mut socket2: TcpStream) -> io::Result<()> {
    // ferry bytes back and forth

    // TODO: wasteful to zero-initialize these.
    let mut buffer1 = [0u8; 2048];
    let mut buffer2 = [0u8; 2048];

    // We can't know who will speak next at any point in time, so we should
    // select! both readers. It's possible for a blocked writer to prevent
    // forward progress, which would imply we should handle the two streams
    // as two separate reader/writer pairs. If we do that, make sure that
    // one pair won't live on forever...

    loop {
        tokio::select! {
            n = socket1.read(&mut buffer1) => {
                let n = n?;
                if n == 0 {
                    println!("EOF on socket1");
                    return Ok(())
                }
                println!("got {} bytes on socket1", n);
                socket2.write(&buffer1).await?;
            }
            n = socket2.read(&mut buffer2) => {
                let n = n?;
                if n == 0 {
                    println!("EOF on socket2");
                    return Ok(())
                }
                println!("got {} bytes on socket2", n);
                socket1.write(&buffer2).await?;
            }
        }
    }

    // TODO: add byte limits, to prevent misuse?
}

// Note: while this returns a Result, if this is spawned directly into the
// executor, meaning that its result will never be seen. If its result is
// interesting, perhaps a logging wrapper should be used.
async fn process_socket(client_socket: TcpStream) -> io::Result<()> {
    println!("got new socket");

    // make a new TCP connection to the target
    let target_host = "localhost";
    let target_port = 443u16;
    // TODO: cache DNS results
    let target_socket = TcpStream::connect((target_host, target_port)).await?;

    proxy(client_socket, target_socket).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    loop {
        let (socket, _socket_addr) = listener.accept().await?;
        // TODO: log socket_addr?
        tokio::spawn(process_socket(socket));
    }
}
