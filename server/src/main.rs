use protocol::{decode, recv_msg, send_msg, ClientToServer, Message, ServerToClient};
use std::collections::HashMap;
use std::env;
use std::io;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug)]
struct Client {
    stream: TcpStream,
}

#[derive(Clone, Debug)]
struct Chat {
    messages: Vec<Message>,
}

struct ServerState {
    clients: HashMap<String, Client>,
    chats: HashMap<(String, String), Chat>,
}

fn normalize_key(s1: &str, s2: &str) -> (String, String) {
    let mut pair = [s1.to_string(), s2.to_string()];
    pair.sort();
    (pair[0].clone(), pair[1].clone())
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let server = {
        if args.len() < 2 {
            "0.0.0.0:8080"
        } else {
            args[1].as_str()
        }
    };
    let listener = TcpListener::bind(server);

    println!("Chat Server listening on:\t{}", server);

    let server_state = Arc::new(Mutex::new(ServerState {
        clients: HashMap::new(),
        chats: HashMap::new(),
    }));

    for stream in listener?.incoming() {
        let stream = stream?;

        println!(
            "Incomming connection from:\t{}\n",
            stream.peer_addr().unwrap()
        );

        //let stream_clone = stream.try_clone()?;
        let state_clone = server_state.clone();

        thread::spawn(move || {
            if let Err(e) = handle_client(stream, state_clone) {
                eprintln!("Error handling client:\t{}", e);
            }
        });
    }

    Ok(())
}
fn send_chat_message(
    state: &Arc<Mutex<ServerState>>,
    handle: &String,
    target: &String,
    content: &String,
) -> io::Result<()> {
    let mut server_state = state.lock().unwrap();

    if !server_state.clients.contains_key(target) {
        // The target handle isn't connected
        let mut client_stream = &server_state.clients.get_mut(handle).unwrap().stream;
        send_msg(
            &mut client_stream,
            &ServerToClient::Error {
                message: "Target handle doesn't exist.".to_string(),
            },
        )?;
        return Ok(());
    }

    let lookup_key = normalize_key(&handle, &target);
    if !server_state.chats.contains_key(&lookup_key) {
        server_state.chats.insert(
            lookup_key.clone(),
            Chat {
                messages: Vec::<Message>::new(),
            },
        );
    }

    let chat = server_state.chats.get_mut(&lookup_key).unwrap();

    chat.messages.push(Message {
        sender: handle.clone(),
        content: content.clone(),
    });

    // Send the message to the target client
    let mut target_stream = &server_state.clients.get_mut(target).unwrap().stream;

    let _ = send_msg(
        &mut target_stream,
        &ServerToClient::ChatMessage {
            sender: handle.clone(),
            content: content.clone(),
        },
    );

    Ok(())
}

fn handle_client(mut stream: TcpStream, state: Arc<Mutex<ServerState>>) -> io::Result<()> {
    // First message should be the client registring with a handle
    let data =
        recv_msg(&mut stream)?.ok_or(io::Error::new(io::ErrorKind::ConnectionReset, "No data"))?;
    let msg: ClientToServer = decode(&data)?;

    let handle = if let ClientToServer::Register { handle } = msg {
        let mut server_state = state.lock().unwrap();
        if server_state.clients.contains_key(&handle) {
            let _ = send_msg(
                &mut stream,
                &ServerToClient::Error {
                    message: "Handle already taken".to_string(),
                },
            );
            return Ok(());
        }
        server_state.clients.insert(
            handle.clone(),
            Client {
                stream: stream.try_clone()?,
            },
        );

        send_msg(
            &mut stream,
            &ServerToClient::Registered {
                handle: handle.clone(),
            },
        )?;
        handle
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Expected Register Command.",
        ));
    };

    loop {
        let data = match recv_msg(&mut stream)? {
            Some(d) => d,
            None => break,
        };

        let msg: ClientToServer = decode(&data)?;

        match msg {
            ClientToServer::Register { handle: _ } => {
                // Should never happen
                todo!();
            }
            ClientToServer::ListUsers => {
                let server_state = state.lock().unwrap();
                let users: Vec<String> = server_state.clients.keys().cloned().collect();
                let _ = send_msg(&mut stream, &ServerToClient::UserList { users })?;
            }
            ClientToServer::SendMessage { content, target } => {
                println!(
                    "Received send message request from {}, '{}' to '{}'\n",
                    handle, content, target
                );
                let _ = send_chat_message(&state, &handle, &target, &content);
            }
            ClientToServer::GetMessages { target } => {
                let server_state = state.lock().unwrap();

                let lookup_key = normalize_key(&handle, &target);
                let messages: Vec<Message> = match server_state.chats.get(&lookup_key) {
                    Some(chat) => chat.messages.clone(),
                    None => Vec::<Message>::new(),
                };

                send_msg(
                    &mut stream,
                    &ServerToClient::ChatMessages {
                        partner: target,
                        messages: messages,
                    },
                )?;
            }
        }
    }
    Ok(())
}
