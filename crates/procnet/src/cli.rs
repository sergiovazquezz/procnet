use std::{io::Write, os::unix::net::UnixStream};

use clap::{Parser, Subcommand};
use procnet_core::{errors::MsgSendError, ipc};
use serde::Serialize;

#[derive(Subcommand, Serialize)]
pub enum DaemonCommand {
    Interval {
        /// Refresh interval in seconds
        interval: f32,
    },
    Reset,
    Status,
}

#[derive(Subcommand)]
pub enum Command {
    Run,
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

const BUF_SIZE: usize = size_of::<u16>() + size_of::<DaemonCommand>();

pub fn send_daemon_command(
    command: &DaemonCommand,
    stream: &mut UnixStream,
) -> Result<(), MsgSendError> {
    let mut buf = Vec::<u8>::with_capacity(BUF_SIZE);

    ipc::write_msg(&mut buf, &command)?;

    stream.write_all(&buf)?;

    Ok(())
}
