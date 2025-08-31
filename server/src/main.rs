use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
//use std::net::SocketAddr;
//use std::os::unix::

#[derive(Debug)]
struct Client {
    stream: TcpStream,
    //addr: SocketAddr,
    current_partner: Option<String>,
}

#[derive(Clone, Debug)]
struct Message {
    content: String,
    sender: String,
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

enum Command {
    ListClients,
    StartChat { target: String },
    ExitChat,
    Unknown
}

fn normalize_key(s1: &str, s2: &str) -> (String, String) {
    let mut pair = [s1.to_string(), s2.to_string()];
    pair.sort(); // Sort to ensure consistent order
    (pair[0].clone(), pair[1].clone())
}

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080");

    println!("Chat Server listening on:\t127.0.0.1:8080");

    //let clients: Arc<Mutex<HashMap<String, TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));

    let server_state = Arc::new(Mutex::new(ServerState {
        clients: HashMap::new(),
        chats: HashMap::new(),
    }));

    let mut client_counter: usize = 0;

    for stream in listener?.incoming() {
        let stream = stream?;

        client_counter += 1;
        let handle = format!("Client_{}", client_counter);

        println!("{}, connected from:\t{}", handle, stream.peer_addr()?);

        let stream_clone = stream.try_clone()?;
        let state_clone = server_state.clone();

        {
            let mut server_state = state_clone.lock().unwrap();
            server_state.clients.insert(
                handle.clone(),
                Client {
                    stream: stream_clone,
                    current_partner: None,
                },
            );
        }

        thread::spawn(move || {
            if let Err(e) = handle_client(stream, state_clone, handle) {
                eprintln!("Error handling client:\t{}", e);
            }
        });
    }

    Ok(())
}

fn get_prompt(state: &Arc<Mutex<ServerState>>, handle: &String) -> String {
    let server_state = state.lock().unwrap();

    if let Some(client) = server_state.clients.get(handle) {
        if let Some(partner) = &client.current_partner {
            return format!("Chat with {}>>", partner);
        }
    }
    return format!("Command ({})>>", handle);
}

fn send_message_with_state(state: &Arc<Mutex<ServerState>>, handle: &str, message: &str) -> io::Result<()> {
    let state = state.lock().unwrap();

    if let Some(client) = state.clients.get(handle) {
        let stream = client.stream.try_clone()?;
        send_message(&stream, message);
    }
    Ok(())
}
fn send_message(stream: &TcpStream, message: &str) -> io::Result<()> {
    let mut stream = stream.try_clone()?;
    stream.write_all(message.as_bytes());
    stream.flush()?;
    Ok(())
}

fn cleanup_client(state: &Arc<Mutex<ServerState>>, handle: &String) -> io::Result<()> {
    println!("Client disconnected:\t{}", handle);
    // TODO send message to people chatting with this handle
    state.lock().unwrap().clients.retain(|k, _v| *k != *handle);

    Ok(())
}

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

fn handle_client(
    stream: TcpStream,
    state: Arc<Mutex<ServerState>>,
    handle: String,
) -> io::Result<()> {
    let mut reader = io::BufReader::new(&stream);

    let mut line = String::new();

    loop {
        let prompt = get_prompt(&state, &handle);

        // Send prompt
        send_message_with_state(&state, &handle, &prompt);

        line.clear();

        let bytes_read = reader.read_line(&mut line)?;

        if bytes_read == 0 {
            cleanup_client(&state, &handle);
            return Ok(());
        }

        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        match parse_command(input.to_string()) {
            Some(cmd) => process_command(&state, &handle, cmd)?,
            None => {
                {
                    let state_locked = state.lock().unwrap();
                    if !state_locked.clients.contains_key(&handle) {
                        continue;
                    }
                }
                send_message_to_partner(&state, &handle, input.to_string())?;
            }
        }
    }
}
