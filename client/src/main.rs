use protocol::{ClientToServer, ServerToClient, send_msg, recv_msg, decode, Message};
use crossterm::{
    event::{self, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
    ExecutableCommand
};
use std::sync::{Arc, Mutex};
use ratatui::{prelude::*, widgets::*};
use std::io;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

//const HELP_MESSAGE: &str = "";

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
    messages: Vec<Message>,
    display: Vec<String>,
    title: String,
    handle: Option<String>,
    input: String,
}

enum Input {
    ListUsers,
    Chat { target: String },
    Exit,
    ChatMessage { message: String },
    InvalidCommand { message: String } ,
    Help,
}

fn parse_input(input: String) -> Input {

    if !input.starts_with('/') {
        // If it is not a command it's a regular message
        return Input::ChatMessage{ message: input };
    }

    let mut parts = input.split_whitespace();

    match parts.next() {
        Some("/users") => Input::ListUsers,
        Some("/chat") => {
            match parts.next() {
                Some(target) => Input::Chat { target: target.to_string() },
                _ => Input::InvalidCommand { message: "No target user requested.".to_string() },
            }
        }
        Some("/exit") => Input::Exit,
        Some("/help") => Input::Help,
        _ => Input::InvalidCommand { message: "Unknown command.".to_string() }
    }
}

fn process_input(client_state: &Arc<Mutex<ClientState>>, mut stream: &TcpStream) -> io::Result<()> {
    let mut state = client_state.lock().unwrap();

    let input = state.input.clone();

    match state.status {
        Status::Initializing => { },
        Status::Registering => {
            // Input is the handle
            let handle = input.trim();

            let _ = send_msg(&mut stream, &ClientToServer::Register { handle: handle.clone().to_string() });

            state.display.push(format!("Requested handle: {}", handle));

        }
        Status::InConsole => {
            // In the main console
            match parse_input(input.trim().to_string()) {
                Input::ListUsers => { send_msg(&mut stream, &ClientToServer::ListUsers); },
                Input::Chat { target } => { send_msg(&mut stream, &ClientToServer::GetMessages { target }); }, 
                Input::Exit => {
                    state.status = Status::Exit;
                }
                Input::ChatMessage { message } => {
                    let _ = state.display.push("[SYSTEM] Please connect to a chat before sending messages.".to_string());
                },
                Input::InvalidCommand { message } => {
                    state.display.push(format!("[SYSTEM] {}", message));
                },
                Input::Help => { },
            }
        }
        Status::InChat => {
            // In a chat 
            match parse_input(input.trim().to_string()) {
                Input::ListUsers => { send_msg(&mut stream, &ClientToServer::ListUsers); },
                Input::Chat { target } => { send_msg(&mut stream, &ClientToServer::GetMessages { target }); }, 
                Input::Exit => {
                    state.status = Status::InConsole;
                    state.display.clear();
                    state.current_partner = None;
                    state.title = format!("Console ({}))", state.handle.clone().unwrap());
                },
                Input::ChatMessage { message } => {
                    if let Some(current_partner) = &state.current_partner {
                        let _ = send_msg(&mut stream, &ClientToServer::SendMessage { content: message.clone(), target: current_partner.to_string() });
                        let handle = state.handle.as_ref().unwrap().to_string();
                        let _ = state.display.push(format!("{}: {}", handle, message.clone()));
                    } else {
                        let _ = state.display.push("[SYSTEM] Please connect to a chat before sending messages.".to_string());
                    }
                },
                Input::InvalidCommand { message } => {
                    state.display.push(format!("[SYSTEM] {}", message));
                },
                Input::Help => { },
            }
        }
        _ => { }
    }


    state.input.clear();

    Ok(())
}

fn draw_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, client_state: &Arc<Mutex<ClientState>>) -> io::Result<()> {

    let (title, display, input) = {
        let state = client_state.lock().unwrap();
        (state.title.clone(), state.display.clone(), state.input.clone())
    };

    terminal.draw(|frame| {
        let area = frame.area();
        let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(90), Constraint::Percentage(10)])
                .split(area);

        let msg_list = List::new(display.iter().map(|m| Line::from(m.as_str())))
            .block(Block::default().title(title).borders(Borders::ALL));
        frame.render_widget(msg_list, chunks[0]);

        let input_para = Paragraph::new(input.as_str())
            .block(Block::default().title("Input").borders(Borders::ALL));
        frame.render_widget(input_para, chunks[1]);
        frame.set_cursor(chunks[1].x + input.len() as u16 + 1, chunks[1].y + 1);
    })?;

    return Ok(())
}

fn listen(state: Arc<Mutex<ClientState>>, mut stream: TcpStream) -> io::Result<()> {

    loop {
        let data = recv_msg(&mut stream)?.ok_or(io::Error::new(io::ErrorKind::ConnectionReset, "No data"))?;

        match decode::<ServerToClient>(&data)? {
            ServerToClient::Registered { handle } => {
                // Successfully registered handle
                let mut st = state.lock().unwrap();
                st.display.push(format!("[SYSTEM] Successfully registered as handle: {}", handle));
                st.title = format!("Console ({})", handle.clone());
                st.handle = Some(handle);
                st.status = Status::InConsole;
            },
            ServerToClient::UserList { users } => {
                // Response with a list of available user handles
                let mut st = state.lock().unwrap();
                st.display.push(format!("[SYSTEM] Available user handles ({})", users.join(",")));
            }
            ServerToClient::Error { message } => {
                let mut st = state.lock().unwrap();
                st.display.push(format!("[SYSTEM] An error occurred: {}", message));
            }
            ServerToClient::ChatMessages { partner, messages } => {
                // The user has requested the chat messages with partner. Enter chat with this user 
                let mut st = state.lock().unwrap();

                st.current_partner = Some(partner.clone());

                st.display.clear();
                st.display.extend(messages.into_iter().map(|m| format!("{}:{}", m.sender, m.content)));
                st.status = Status::InChat;
                st.title = format!("In Chat with '{}'", partner);
            },
            ServerToClient::ChatMessage { sender, content } => {
                let mut st = state.lock().unwrap();

                if st.current_partner.as_ref().map_or(false, |s| *s == sender) {
                    st.display.push(format!("{}: {}", sender, content));
                } else {
                    // TODO add a system message that someone has sent a message to the user
                }
            }
//    Registered { handle: String },
//    UserList { users: Vec<String> },
//    ChatMessages { partner: String, messages: Vec<Message> },
//    ChatMessage { sender: String, content: String },
//   Error { message: String },
        }

    }
}

fn render(client_state: Arc<Mutex<ClientState>>) -> io::Result<()> {

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    loop {
        let _ = draw_terminal(&mut terminal, &client_state);
        thread::sleep(Duration::from_millis(32));
    }

    Ok(())
}

fn main() -> io::Result<()> {

    enable_raw_mode()?;

    io::stdout().execute(crossterm::terminal::EnterAlternateScreen)?;

    let mut stream = TcpStream::connect("127.0.0.1:8080")?;

    let client_state = Arc::new(Mutex::new(ClientState {
        status: Status::Initializing,
        messages: Vec::<Message>::new(),
        display: Vec::<String>::new(),
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
                    state.display.push("Please enter your handle...".to_string());
                    state.status = Status::Registering;
                },
                Status::Exit => break,
                _ => { }
            }
        }

        if let event::Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Enter =>   {
                    let _ = process_input(&client_state, &stream);
                }
                KeyCode::Char(c) => {
                    let mut state = client_state.lock().unwrap();
                    state.input.push(c);
                }
                KeyCode::Backspace => {
                    let mut state = client_state.lock().unwrap();
                    state.input.pop();
                },
                KeyCode::Esc => break,
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    io::stdout().execute(crossterm::terminal::LeaveAlternateScreen)?;

    Ok(())
}
