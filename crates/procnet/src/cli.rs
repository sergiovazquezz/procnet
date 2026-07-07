use std::{io::Write, os::unix::net::UnixStream};

use clap::{Parser, Subcommand};
use procnet_core::{
    errors::MsgSendError,
    ipc::{self, DaemonCommand},
};

const BUF_SIZE: usize = size_of::<u16>() + size_of::<DaemonCommand>();

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

pub fn send_daemon_command(
    command: DaemonCommand,
    stream: &mut UnixStream,
) -> Result<(), MsgSendError> {
    let mut buf = Vec::<u8>::with_capacity(BUF_SIZE);

    ipc::write_msg(&mut buf, &command)?;

    stream.write_all(&buf)?;

    Ok(())
}
