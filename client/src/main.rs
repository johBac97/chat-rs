use protocol::{ClientToServer, ServerToClient, send_msg, recv_msg, decode};
use std::io;
use std::net::TcpStream;
use std::thread;

fn main() -> io::Result<()> {
    let mut stream = TcpStream::connect("127.0.0.1:8080")?;
    let mut stream_clone = stream.try_clone()?;

    println!("Enter your handle");
    let mut handle = String::new();
    io::stdin().read_line(&mut handle)?;
    handle = handle.trim().to_string();

    let _ = send_msg(&mut stream, &ClientToServer::Register { handle: handle.clone() })?;

    let data = recv_msg(&mut stream)?.ok_or(io::Error::new(io::ErrorKind::ConnectionReset, "No response"))?;

    if let Ok(response) = decode(&data) {
        match response {
            ServerToClient::Registered { handle } => {
                println!("Successfully registered handle:\t{}\n", handle);
            }
            ServerToClient::Error { message } => {
                println!("An error occurred:\t{}\n", message);
                return Err(io::Error::new(io::ErrorKind::InvalidData, "An error occurred"));
            }
        _ => {
            todo!("Shouldn't happen");
        }
        };
    }

    thread::spawn(move || {
        loop {
            let data = match recv_msg(&mut stream_clone) {
                Ok(Some(d)) => d,
                _ => break,

            };

            match decode::<ServerToClient>(&data) {
                Ok(msg) => {
                    match msg {
                        ServerToClient::ChatMessage { sender, content } => println!("{}: {}\n", sender, content),
                        ServerToClient::UserList { users } => println!("Users: {:?}\n", users),
                        _ => println!("Received: {:?}\n", msg),
                    }
                }
                Err(e) => {
                    println!("An error occurred:\t{}\n", e);
                    continue;
                }
            }
        }
    });

    let stdin = io::stdin();
    let mut target_handle: Option<String> = None;

    for line in stdin.lines() {
        let input = line?.trim().to_string();
        if input.is_empty() { continue; }

        let msg = if input.starts_with('/') {
            // command
            if input == "/users" {
                Some(ClientToServer::ListUsers)
            } else if input.starts_with("/chat ") {
                target_handle = input.split_whitespace().nth(1).map(|s| s.to_string());
                if let Some(target) = &target_handle {
                    Some(ClientToServer::GetMessages { target: target.clone() })
                } else {
                    println!("Invalid chat target handle.\n");
                    None
                }
            } else if input == "/exit" {
                target_handle = None;
                None
            } else {
                println!("Unknown command:\t{}\n", input);
                None
            }
        } else {
            // Normal message
            if let Some(target) = &target_handle {
                Some(ClientToServer::SendMessage { content: input, target: target.clone() })
            } else {
                println!("Cannot send chat message when not in chat.\n");
                None
            }
        };

        if let Some(message) = msg {
            send_msg(&mut stream, &message)?;
        }
    }

    Ok(())
}
