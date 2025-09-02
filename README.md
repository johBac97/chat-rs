# Rust Chat Application

This is an educational project to learn Rust, focusing on building a simple, concurrent TCP-based chat application. 

## Project Structure

The project is split into three main components:
- **Server**: Handles client connections, message routing, and user management, using a shared protocol package for message serialization.
- **Client**: A Ratatui-based TUI that connects to the server, sends user messages, and displays incoming messages.
- **Protocol**: A shared library defining the message format (including JSON or bincode serialization) used by both server and client.

Both server and client use threading to handle concurrent reading and writing of messages over a single `TcpStream` (cloned for read/write). The client uses `Arc<Mutex>` for shared state to update the UI and process messages simultaneously.

## Installation and Running

### Prerequisites
- Rust and Cargo (install via `rustup`: https://rustup.rs/)

### Build
Clone the repository and build with Cargo:
```bash
git clone https://github.com/johBac97/chat-rs.git 
cd chat-rs
cargo build --release
```

### Run the Server
Start the server, specifying the URI to bind to (e.g., `0.0.0.0:8080`):
```bash
cargo run --bin server -- 0.0.0.0:8080
```

### Run the Client
Start the client, specifying the server’s address (e.g., `127.0.0.1:8080` for localhost):
```bash
cargo run --bin client -- 127.0.0.1:8080
```
