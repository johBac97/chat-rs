use serde::{Deserialize, Serialize};
use std::io::{Write, self, Read};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ClientToServer {
    Register { handle: String },
    ListUsers, 
    SendMessage { content: String, target: String },
    GetMessages { target: String },
    //ExitChat,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerToClient {
    Registered { handle: String },
    UserList { users: Vec<String> },
    //ChatStarted { partner: String, history: Vec<Message> },
    ChatMessages { partner: String, messages: Vec<Message> },
    ChatMessage { sender: String, content: String },
    Error { message: String },
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub sender: String,
    pub content: String
}

#[cfg(feature = "json")]
macro_rules! serialize {
    ($msg:expr) => {{
        serde_json::to_vec($msg)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
    }};
}

#[cfg(feature = "bincode")]
macro_rules! serialize {
    ($msg:expr) => {{
        bincode::serialize($msg)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
    }};
}

#[cfg(feature = "json")]
macro_rules! deserialize {
    ($data:expr) => {{
        serde_json::from_slice($data)
    }};
}

#[cfg(feature = "bincode")]
macro_rules! deserialize {
    ($data:expr) => {{
        bincode::deserialize($data)
    }};
}


pub fn send_msg<W: Write, T: Serialize>(writer: &mut W, msg: &T) -> io::Result<()> {
    let data = serialize!(msg);

    let len = data.len() as u32;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(&data)?;
    writer.flush()?;

    Ok(())
}

pub fn recv_msg<R: Read>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut len_bytes = [0u8; 4];

    // Every message is prefixed with the number of bytes the message is
    if let Err(e) = reader.read_exact(&mut len_bytes) {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            return Ok(None);
        }
        return Err(e);
    }

    let len = u32::from_be_bytes(len_bytes) as usize;
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data)?;

    Ok(Some(data))
}

pub fn decode<T: for<'de> Deserialize<'de>>(data: &[u8]) -> io::Result<T> {
    let result = deserialize!(data);
    result.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
