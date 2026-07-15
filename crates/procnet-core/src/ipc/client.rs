use std::{env, io, os::unix::net::UnixStream, path::PathBuf};

use crate::{
    errors::ConnectError,
    ipc::{SOCKET_FILENAME, SYSTEM_SOCKET_PATH},
};

pub fn connect_to_socket() -> Result<UnixStream, ConnectError> {
    let candidates = socket_paths();
    let mut last_err: Option<io::Error> = None;

    for path in &candidates {
        match UnixStream::connect(path) {
            Ok(stream) => return Ok(stream),
            Err(err) => last_err = Some(err),
        }
    }

    Err(ConnectError(last_err.unwrap_or_else(|| {
        io::Error::new(io::ErrorKind::AddrNotAvailable, "No socket candidates")
    })))
}

#[must_use]
fn socket_paths() -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(2);

    if let Some(dir) = env::var_os("XDG_RUNTIME_DIR")
        && !dir.is_empty()
    {
        paths.push(PathBuf::from(dir).join(SOCKET_FILENAME));
    }

    paths.push(PathBuf::from(SYSTEM_SOCKET_PATH));

    paths
}
