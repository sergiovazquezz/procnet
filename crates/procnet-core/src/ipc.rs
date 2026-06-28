use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
};

use serde::{Deserialize, Serialize};

use crate::{
    errors::{ConnectError, MsgReadError, MsgSendError},
    stats::StatsRow,
};

pub const DEFAULT_SOCKET_PATH: &str = "/tmp/procnetd.sock";

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SnapshotData {
    pub tick: u64,
    pub rows: Vec<StatsRow>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "kind")]
pub enum Message {
    Snapshot(SnapshotData),
    Error(String),
}

pub fn connect_to_socket() -> Result<UnixStream, ConnectError> {
    let stream = UnixStream::connect(DEFAULT_SOCKET_PATH)?;
    Ok(stream)
}

pub fn send_msg(stream: &mut UnixStream, msg: &Message) -> Result<(), MsgSendError> {
    serde_json::to_writer(&mut *stream, msg)?;
    stream.write_all(b"\n")?;
    Ok(())
}

pub fn read_msg(reader: &mut BufReader<UnixStream>) -> Result<Message, MsgReadError> {
    let mut line = String::new();

    if reader.read_line(&mut line)? == 0 {
        return Err(MsgReadError::Eof);
    }

    let response: Message = serde_json::from_str(&line)?;

    Ok(response)
}
