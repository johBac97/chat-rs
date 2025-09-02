use crossterm::{
    event::{self, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
    ExecutableCommand,
};
use protocol::{decode, recv_msg, send_msg, ClientToServer, ServerToClient};
use ratatui::{prelude::*, widgets::*};
use std::env;
use std::io;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const HELP_MESSAGE: &str = "Welcome to Chat-rs. These are the available commands:
    '/users': Display available users.
    '/chat <user>': Enter a chat with a target user.
    '/exit': Exit a chat or Chat-rs itself.
    '/help': Display this help message.";

enum Status {
    Initializing,
    Registering,
    InConsole,
    InChat,
    Exit,
}

struct ClientState {
    status: Status,
    current_partner: Option<String>,
    display: Vec<DisplayMessage>,
    title: String,
    handle: Option<String>,
    input: String,
}

enum Input {
    ListUsers,
    Chat { target: String },
    Exit,
    ChatMessage { message: String },
    InvalidCommand { message: String },
    Help,
}

#[derive(Clone)]
enum DisplayMessageMode {
    System,
    User,
    OtherUser,
}

#[derive(Clone)]
struct DisplayMessage {
    content: String,
    sender: String,
    mode: DisplayMessageMode,
}

fn parse_input(input: String) -> Input {
    if !input.starts_with('/') {
        // If it is not a command it's a regular message
        return Input::ChatMessage { message: input };
    }

    let mut parts = input.split_whitespace();

    match parts.next() {
        Some("/users") => Input::ListUsers,
        Some("/chat") => match parts.next() {
            Some(target) => Input::Chat {
                target: target.to_string(),
            },
            _ => Input::InvalidCommand {
                message: "No target user requested.".to_string(),
            },
        },
        Some("/exit") => Input::Exit,
        Some("/help") => Input::Help,
        _ => Input::InvalidCommand {
            message: "Unknown command.".to_string(),
        },
    }
}

fn process_input(client_state: &Arc<Mutex<ClientState>>, mut stream: &TcpStream) -> io::Result<()> {
    let mut state = client_state.lock().unwrap();

    let input = state.input.clone();

    match state.status {
        Status::Initializing => {}
        Status::Registering => {
            // Input is the handle
            let handle = input.trim();

            let _ = send_msg(
                &mut stream,
                &ClientToServer::Register {
                    handle: handle.to_string(),
                },
            );

            state.display.push(DisplayMessage {
                content: format!("Requested handle: {}", handle),
                sender: "System".to_string(),
                mode: DisplayMessageMode::System,
            });

            state.display.extend(
                String::from(HELP_MESSAGE)
                    .split("\n")
                    .map(|l| DisplayMessage {
                        content: String::from(l),
                        sender: "System".to_string(),
                        mode: DisplayMessageMode::System,
                    }),
            );
        }
        Status::InConsole => {
            // In the main console
            match parse_input(input.trim().to_string()) {
                Input::ListUsers => {
                    let _ = send_msg(&mut stream, &ClientToServer::ListUsers);
                }
                Input::Chat { target } => {
                    let _ = send_msg(&mut stream, &ClientToServer::GetMessages { target });
                }
                Input::Exit => {
                    state.status = Status::Exit;
                }
                Input::ChatMessage { message: _message } => {
                    state.display.push(DisplayMessage {
                        content: "Please connect to a chat to send a message.".to_string(),
                        sender: "System".to_string(),
                        mode: DisplayMessageMode::System,
                    });
                }
                Input::InvalidCommand { message } => {
                    state.display.push(DisplayMessage {
                        content: message,
                        sender: "System".to_string(),
                        mode: DisplayMessageMode::System,
                    });
                }
                Input::Help => {
                    state
                        .display
                        .extend(
                            String::from(HELP_MESSAGE)
                                .split("\n")
                                .map(|l| DisplayMessage {
                                    content: String::from(l),
                                    sender: "System".to_string(),
                                    mode: DisplayMessageMode::System,
                                }),
                        );
                }
            }
        }
        Status::InChat => {
            // In a chat
            match parse_input(input.trim().to_string()) {
                Input::ListUsers => {
                    send_msg(&mut stream, &ClientToServer::ListUsers)?;
                }
                Input::Chat { target } => {
                    send_msg(&mut stream, &ClientToServer::GetMessages { target })?;
                }
                Input::Exit => {
                    state.status = Status::InConsole;
                    state.display.clear();
                    state.current_partner = None;
                    state.title = format!("Console ({}))", state.handle.clone().unwrap());
                }
                Input::ChatMessage { message } => {
                    if let Some(current_partner) = &state.current_partner {
                        let _ = send_msg(
                            &mut stream,
                            &ClientToServer::SendMessage {
                                content: message.clone(),
                                target: current_partner.to_string(),
                            },
                        );
                        let handle = state.handle.as_ref().unwrap().to_string();
                        state.display.push(DisplayMessage {
                            content: message.clone(),
                            sender: handle,
                            mode: DisplayMessageMode::User,
                        });
                    } else {
                        state.display.push(DisplayMessage {
                            content: "Please connect to a chat before sending a message."
                                .to_string(),
                            sender: "System".to_string(),
                            mode: DisplayMessageMode::System,
                        });
                    }
                }
                Input::InvalidCommand { message } => {
                    state.display.push(DisplayMessage {
                        content: message,
                        sender: "System".to_string(),
                        mode: DisplayMessageMode::System,
                    });
                }
                Input::Help => {
                    state
                        .display
                        .extend(
                            String::from(HELP_MESSAGE)
                                .split("\n")
                                .map(|l| DisplayMessage {
                                    content: String::from(l),
                                    sender: "System".to_string(),
                                    mode: DisplayMessageMode::System,
                                }),
                        );
                }
            }
        }
        _ => {}
    }

    state.input.clear();

    Ok(())
}

fn draw_terminal(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    client_state: &Arc<Mutex<ClientState>>,
) -> io::Result<()> {
    let (title, display, input) = {
        let state = client_state.lock().unwrap();
        (
            state.title.clone(),
            state.display.clone(),
            state.input.clone(),
        )
    };

    let mut list_state = ListState::default();

    terminal.draw(|frame| {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(90), Constraint::Percentage(10)])
            .split(area);

        if !display.is_empty() {
            list_state.select(Some(display.len().saturating_sub(1)));
        } else {
            list_state.select(None);
        }

        let msg_list = List::new(display.iter().map(|m| {
            let sender = format!("[{}] ", m.sender.to_uppercase());

            let sender_formatted = match m.mode {
                DisplayMessageMode::User => sender.green().bold(),
                DisplayMessageMode::OtherUser => sender.blue().bold(),
                DisplayMessageMode::System => sender.red().bold(),
            };

            Line::from(vec![sender_formatted, m.content.as_str().into()])
        }))
        .block(Block::default().title(title).borders(Borders::ALL));
        frame.render_stateful_widget(msg_list, chunks[0], &mut list_state);

        let input_para = Paragraph::new(input.as_str())
            .block(Block::default().title("Input").borders(Borders::ALL));
        frame.render_widget(input_para, chunks[1]);
        frame.set_cursor_position(Position::new(
            chunks[1].x + input.len() as u16 + 1,
            chunks[1].y + 1,
        ));
    })?;

    Ok(())
}

fn listen(state: Arc<Mutex<ClientState>>, mut stream: TcpStream) -> io::Result<()> {
    loop {
        let data = recv_msg(&mut stream)?
            .ok_or(io::Error::new(io::ErrorKind::ConnectionReset, "No data"))?;

        match decode::<ServerToClient>(&data)? {
            ServerToClient::Registered { handle } => {
                // Successfully registered handle
                let mut st = state.lock().unwrap();
                st.display.push(DisplayMessage {
                    content: format!("Successfully registered as user: {}", handle),
                    sender: "System".to_string(),
                    mode: DisplayMessageMode::System,
                });
                st.title = format!("Console ({})", handle.clone());
                st.handle = Some(handle);
                st.status = Status::InConsole;
            }
            ServerToClient::UserList { users } => {
                // Response with a list of available user handles
                let mut st = state.lock().unwrap();
                st.display.push(DisplayMessage {
                    content: format!("Available users: {}", users.join(", ")),
                    sender: "System".to_string(),
                    mode: DisplayMessageMode::System,
                });
            }
            ServerToClient::Error { message } => {
                let mut st = state.lock().unwrap();
                st.display.push(DisplayMessage {
                    content: format!("An error occurred: {}", message),
                    sender: "System".to_string(),
                    mode: DisplayMessageMode::System,
                });
            }
            ServerToClient::ChatMessages { partner, messages } => {
                // The user has requested the chat messages with partner. Enter chat with this user
                let mut st = state.lock().unwrap();

                st.current_partner = Some(partner.clone());

                st.display.clear();
                st.display
                    .extend(messages.into_iter().map(|m| DisplayMessage {
                        content: m.content,
                        mode: if m.sender == partner {
                            DisplayMessageMode::OtherUser
                        } else if m.sender == "System" {
                            DisplayMessageMode::System
                        } else {
                            DisplayMessageMode::User
                        },
                        sender: m.sender,
                    }));
                st.status = Status::InChat;
                st.title = format!("In Chat with '{}'", partner);
            }
            ServerToClient::ChatMessage { sender, content } => {
                let mut st = state.lock().unwrap();

                if st.current_partner.as_ref().map_or(false, |s| *s == sender) {
                    st.display.push(DisplayMessage {
                        content,
                        sender,
                        mode: DisplayMessageMode::OtherUser,
                    });
                } else {
                    // TODO limit this to prevent excessive spam
                    st.display.push( DisplayMessage {
                        content: format!("{} just sent you a message. Join the chat using the command '/chat {}'", sender, sender),
                        sender: "System".to_string(),
                        mode: DisplayMessageMode::System,

                    });
                }
            }
        }
    }
}

fn render(client_state: Arc<Mutex<ClientState>>) -> io::Result<()> {
    enable_raw_mode()?;
    io::stdout().execute(crossterm::terminal::EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    loop {
        draw_terminal(&mut terminal, &client_state)?;
        thread::sleep(Duration::from_millis(32));
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let server = {
        if args.len() < 2 {
            "127.0.0.1:8080"
        } else {
            args[1].as_str()
        }
    };

    let stream = TcpStream::connect(server)?;

    let client_state = Arc::new(Mutex::new(ClientState {
        status: Status::Initializing,
        display: Vec::<DisplayMessage>::new(),
        title: "Connecting...".to_string(),
        handle: None,
        current_partner: None,
        input: String::new(),
    }));

    let client_clone_data = client_state.clone();
    let stream_clone_data = stream.try_clone()?;

    thread::spawn(move || {
        if let Err(e) = listen(client_clone_data, stream_clone_data) {
            eprintln!("An error occurred in data receiving thread: {}", e);
        }
    });

    let client_clone_render = client_state.clone();

    thread::spawn(move || {
        if let Err(e) = render(client_clone_render) {
            eprintln!("An error occurred in screen rendering thread: {}", e);
        }
    });

    loop {
        {
            let mut state = client_state.lock().unwrap();
            match state.status {
                Status::Initializing => {
                    // Not registred yet, do that first.
                    state.title = "Registering".to_string();
                    state.display.push(DisplayMessage {
                        content: "Please enter your user name...".to_string(),
                        sender: "System".to_string(),
                        mode: DisplayMessageMode::System,
                    });
                    state.status = Status::Registering;
                }
                Status::Exit => break,
                _ => {}
            }
        }

        if let event::Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Enter => {
                    let _ = process_input(&client_state, &stream);
                }
                KeyCode::Char(c) => {
                    let mut state = client_state.lock().unwrap();
                    state.input.push(c);
                }
                KeyCode::Backspace => {
                    let mut state = client_state.lock().unwrap();
                    state.input.pop();
                }
                KeyCode::Esc => break,
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    io::stdout().execute(crossterm::terminal::LeaveAlternateScreen)?;

    Ok(())
}
