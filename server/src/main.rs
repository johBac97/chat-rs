use protocol::{decode, recv_msg, send_msg, ClientToServer, Message, ServerToClient};
use std::collections::HashMap;
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
    // participants: Vec<&Client>,
    messages: Vec<Message>,
}

struct ServerState {
    clients: HashMap<String, Client>,
    chats: HashMap<(String, String), Chat>,
}

/*
enum Command {
    ListClients,
    StartChat { target: String },
    ExitChat,
    Unknown
}
*/

fn normalize_key(s1: &str, s2: &str) -> (String, String) {
    let mut pair = [s1.to_string(), s2.to_string()];
    pair.sort(); // Sort to ensure consistent order
    (pair[0].clone(), pair[1].clone())
}

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080");

    println!("Chat Server listening on:\t127.0.0.1:8080");

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

/*
fn cleanup_client(state: &Arc<Mutex<ServerState>>, handle: &String) -> io::Result<()> {
    println!("Client disconnected:\t{}", handle);
    // TODO send message to people chatting with this handle
    state.lock().unwrap().clients.retain(|k, _v| *k != *handle);

    Ok(())
}
*/

/*
fn parse_command(input: String) -> Option<Command> {
    let mut parts = input.split_whitespace();
    let cmd = parts.next()?;

    if !cmd.starts_with('/') {
        return None;
    }

    match cmd {
        "/clients" => Some(Command::ListClients),
        "/chat" => {
            if let Some(handle) = parts.next() {
                Some(Command::StartChat {
                    target: handle.to_string(),
                })
            } else {
                None
            }
        }
        "/exit" => Some(Command::ExitChat),
        _ => Some(Command::Unknown)
    }
}
fn process_command(
    state: &Arc<Mutex<ServerState>>,
    handle: &String,
    command: Command,
) -> io::Result<()> {
    match command {
        Command::ListClients => {
            let msg = {
                let server_state = state.lock().unwrap();
                let handles: Vec<String> = server_state.clients.keys().cloned().collect();
                format!("Clients:\n{}\n", handles.join("\n"))
            };
            send_message_with_state(state, handle, &msg);
        }
        Command::StartChat { target } => {
            let mut server_state = state.lock().unwrap();
            let client_stream = &server_state.clients.get(handle).unwrap().stream;

            // Check if partner_handle exists
            if !server_state.clients.contains_key(&target) {
                return send_message(client_stream, "Partner not found!\n");
            }

            if target == *handle {
                return send_message(client_stream, "Cannot chat with yourself.\n");
            }

            let chat_lookup = normalize_key(handle, &target);
            if let Some(chat) = server_state.chats.get(&chat_lookup) {
                // There is already a chat
                let message_history: String = chat
                    .messages
                    .clone()
                    .into_iter()
                    .map(|m| format!("{}: {}\n", m.sender, m.content))
                    .collect::<Vec<_>>()
                    .join("\n");

                send_message(client_stream, &message_history);
            } else {
                // New chat
                server_state.chats.insert(
                    chat_lookup.clone(),
                    Chat {
                        messages: Vec::new(),
                    },
                );

            }
            // Assign chat partner
            server_state
                .clients
                .get_mut(handle.as_str())
                .unwrap()
                .current_partner = Some(target);
        }
        Command::ExitChat => todo!(),
        Command::Unknown => {
            send_message_with_state(state, handle, "Unknown command\n");
        }
    }

    Ok(())
}
*/

/*
fn send_message_to_partner(
    state: &Arc<Mutex<ServerState>>,
    handle: &String,
    message: String,
) -> io::Result<()> {
    let mut server_state = state.lock().unwrap();

    let current_partner = {
        let client = match server_state.clients.get(handle) {
            Some(client) => client,
            None => {
                println!("Client not found:\t{}\n", handle);
                return Ok(());
            }
        };

        match &client.current_partner {
            Some(partner) => partner.clone(),
            None => {
                println!("Client {} has no assign current partern.\n", handle);
                return Ok(());
            }
        }
    };

    let chat_lookup = normalize_key(handle, current_partner.as_str());

    let chat = match server_state.chats.get_mut(&chat_lookup) {
        Some(chat) => chat,
        None => {
            println!("Chat not found");
            return Ok(())
        }
    };

    // Add message to chat
    chat.messages.push(Message {
        content: message.clone(),
        sender: (*handle).clone(),
    });

    if let Some(partner_client) = server_state.clients.get(&current_partner) {
        if let Some(partner_client_partner) = &partner_client.current_partner {
            if *partner_client_partner == *handle {
                send_message(&partner_client.stream, &message);
            }
        }
    }


    Ok(())
}
*/

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
        server_state
            .chats
            .insert(
                lookup_key.clone(),
                Chat {
                    messages: Vec::<Message>::new(),
                },
            )
            .unwrap();
    }

    let chat = server_state.chats.get_mut(&lookup_key).unwrap();

    chat.messages.push(Message {
        sender: handle.clone(),
        content: content.clone(),
    });

    // Send the message to the target client
    let mut target_stream = &server_state.clients.get_mut(target).unwrap().stream;
    send_msg(
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
            send_msg(
                &mut stream,
                &ServerToClient::Error {
                    message: "Handle already taken".to_string(),
                },
            );
            return Ok(());
        }
        server_state
            .clients
            .insert(
                handle.clone(),
                Client {
                    stream: stream.try_clone()?,
                },
            )
            .unwrap();
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
                send_msg(&mut stream, &ServerToClient::UserList { users })?;
            }
            ClientToServer::SendMessage { content, target } => {
                send_chat_message(&state, &handle, &target, &content);
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
