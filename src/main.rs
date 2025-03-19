mod config;

use crate::config::Server;
use futures::FutureExt;
use std::error::Error;
use std::net::SocketAddr;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::task;

type BoxedError = Box<dyn Error + Sync + Send + 'static>;
const BUF_SIZE: usize = 2048;

#[tokio::main]
async fn main() -> Result<(), BoxedError> {
    config::init_config();
    let mut handles = vec![];
    let servers = &config::get_config().forward;
    for server in servers.iter() {
        let handle = task::spawn(async move {
            if let Err(e) = forward(&server).await {
                eprintln!("Error in forward: {}", e);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    Ok(())
}

async fn forward(server: &Server) -> Result<(), BoxedError> {
    let bind_sock = server
        .listen_address
        .parse::<SocketAddr>()
        .expect("Failed to parse bind address");
    let listener = TcpListener::bind(&bind_sock).await?;
    println!(
        "Forwarding {} to {} ({})",
        listener.local_addr().unwrap(),
        server.forward_address,
        server.name
    );

    // We leak `remote` instead of wrapping it in an Arc to share it with future tasks since
    // `remote` is going to live for the lifetime of the server in all cases.
    // (This reduces MESI/MOESI cache traffic between CPU cores.)
    let remote: &str = Box::leak(server.forward_address.clone().into_boxed_str());

    loop {
        let (mut client, client_addr) = listener.accept().await?;

        tokio::spawn(async move {
            println!("New connection from {}", client_addr);

            // Establish connection to upstream for each incoming client connection
            let mut remote = match TcpStream::connect(remote).await {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("Error establishing upstream connection: {e}");
                    return;
                }
            };
            let (mut client_read, mut client_write) = client.split();
            let (mut remote_read, mut remote_write) = remote.split();

            let (cancel, _) = broadcast::channel::<()>(1);
            let (remote_copied, client_copied) = tokio::join! {
                copy_with_abort(&mut remote_read, &mut client_write, cancel.subscribe())
                    .then(|r| { let _ = cancel.send(()); async { r } }),
                copy_with_abort(&mut client_read, &mut remote_write, cancel.subscribe())
                    .then(|r| { let _ = cancel.send(()); async { r } }),
            };

            match client_copied {
                Ok(count) => {
                    if config::get_config().debug {
                        eprintln!(
                            "Transferred {} bytes from proxy client {} to upstream server",
                            count, client_addr
                        );
                    }
                }
                Err(err) => {
                    eprintln!(
                        "Error writing bytes from proxy client {} to upstream server",
                        client_addr
                    );
                    eprintln!("{}", err);
                }
            };

            match remote_copied {
                Ok(count) => {
                    if config::get_config().debug {
                        eprintln!(
                            "Transferred {} bytes from upstream server to proxy client {}",
                            count, client_addr
                        );
                    }
                    println!("Closed connection of {}", client_addr);
                }
                Err(err) => {
                    eprintln!(
                        "Error writing from upstream server to proxy client {}!",
                        client_addr
                    );
                    eprintln!("{}", err);
                }
            };
        });
    }

    // Two instances of this function are spawned for each half of the connection: client-to-server,
    // server-to-client.
    async fn copy_with_abort<R, W>(
        read: &mut R,
        write: &mut W,
        mut abort: broadcast::Receiver<()>,
    ) -> tokio::io::Result<usize>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let mut copied = 0;
        let mut buf = [0u8; BUF_SIZE];
        loop {
            let bytes_read;
            tokio::select! {
                biased;

                result = read.read(&mut buf) => {
                    use std::io::ErrorKind::{ConnectionReset, ConnectionAborted};
                    bytes_read = result.or_else(|e| match e.kind() {
                        // Consider these to be part of the proxy life, not errors
                        ConnectionReset | ConnectionAborted => Ok(0),
                        _ => Err(e)
                    })?;
                },
                _ = abort.recv() => {
                    break;
                }
            }

            if bytes_read == 0 {
                break;
            }

            // any error writing data we've already read to the other side is treated as exceptional.
            write.write_all(&buf[0..bytes_read]).await?;
            copied += bytes_read;
        }

        Ok(copied)
    }
}
