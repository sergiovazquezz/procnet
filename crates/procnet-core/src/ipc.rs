use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::stats::StatsRow;

pub const DEFAULT_SOCKET_PATH: &str = "/run/procnetd.sock";

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SnapshotData {
    pub tick: u64,
    pub dropped: u64,
    pub rows: Vec<StatsRow>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "kind")]
pub enum Message {
    Snapshot(SnapshotData),
    Error(String),
}

pub fn connect_to_socket() -> Result<UnixStream> {
    UnixStream::connect(DEFAULT_SOCKET_PATH).context(format!(
        "Failed to connect to socket at {}",
        DEFAULT_SOCKET_PATH
    ))
}

pub fn send_msg(stream: &mut UnixStream, msg: &Message) -> Result<()> {
    serde_json::to_writer(&mut *stream, msg)?;
    stream.write_all(b"\n")?;
    Ok(())
}

pub fn read_msg(reader: &mut BufReader<UnixStream>) -> Result<Message> {
    let mut line = String::new();

    let n = reader.read_line(&mut line)?;
    if n == 0 {
        return Err(anyhow!("eof reached, daemon closed"));
    }

    let response: Message = serde_json::from_str(&line)?;

    Ok(response)
}
