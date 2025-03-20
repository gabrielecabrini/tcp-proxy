# TCP Proxy

This project is a simple, configurable TCP proxy that forwards traffic from one server to another. It is built using Rust and the `tokio` async runtime, which allows it to handle multiple concurrent connections efficiently.

## Features

- **TCP Forwarding**: Listens for incoming TCP connections and forwards them to a specified upstream server.
- **Configuration**: Configurable via a YAML file (`config.yml`) that specifies the servers to forward traffic to, along with their listen and forward addresses.
- **Debug Mode**: Debug logs for tracking data transfer and connection events.
- **Graceful Connection Handling**: Ensures connections are forwarded while handling errors like connection resets or aborts.
- **Async Architecture**: Fully asynchronous implementation using `tokio` for handling multiple connections concurrently.
