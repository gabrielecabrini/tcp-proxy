mod config;

use crate::config::Server;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, copy_bidirectional};
use tokio::net::{TcpListener, TcpStream};
use tokio::task;

type BoxedError = Box<dyn Error + Sync + Send + 'static>;

#[tokio::main]
async fn main() -> Result<(), BoxedError> {
    config::init_config();
    let servers = &config::get_config().forward;

    let mut handles = Vec::new();

    for server in servers.iter() {
        let server = server.clone();
        let handle = task::spawn(async move {
            if let Err(e) = setup_forward(&server).await {
                eprintln!("Error in forward server {}: {}", server.name, e);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    Ok(())
}

async fn setup_forward(server: &Server) -> Result<(), BoxedError> {
    let bind_addr: SocketAddr = server
        .listen_address
        .parse()
        .expect("Failed to parse bind address");

    let listener = TcpListener::bind(&bind_addr).await?;
    println!(
        "Forwarding {} to {} ({})",
        listener.local_addr()?,
        server.forward_address,
        server.name
    );

    let remote = Arc::new(server.forward_address.clone());
    let server_name = Arc::new(server.name.clone());

    loop {
        let (mut client, client_addr) = listener.accept().await?;
        let remote = remote.clone();
        let server_name = server_name.clone();

        tokio::spawn(async move {
            println!("New connection from {} (forwarding to {})", client_addr, server_name);

            match TcpStream::connect(&*remote).await {
                Ok(mut upstream) => {
                    if let Err(e) = copy_bidirectional(&mut client, &mut upstream).await {
                        eprintln!("Error during forwarding ({server_name}): {e}");
                    }

                    // Close both ends explicitly
                    let _ = client.shutdown().await;
                    let _ = upstream.shutdown().await;

                    println!("Closed connection of {} ({server_name})", client_addr);
                }
                Err(e) => {
                    eprintln!("Error connecting to upstream ({server_name}): {e}");
                }
            }
        });
    }
}
