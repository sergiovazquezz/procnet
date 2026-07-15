use std::{env, ffi, path::PathBuf};

use crate::ipc::{SOCKET_FILENAME, SYSTEM_SOCKET_PATH};

#[must_use]
pub fn socket_path() -> PathBuf {
    resolve_socket_path(env::var_os("XDG_RUNTIME_DIR").as_deref())
}

fn resolve_socket_path(runtime_var: Option<&ffi::OsStr>) -> PathBuf {
    runtime_var.filter(|v| !v.is_empty()).map_or_else(
        || PathBuf::from(SYSTEM_SOCKET_PATH),
        |dir| PathBuf::from(dir).join(SOCKET_FILENAME),
    )
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    #[test]
    fn socket_path_uses_xdg_when_set() {
        let p = resolve_socket_path(Some(OsStr::new("/run/user/1000")));
        assert_eq!(p, PathBuf::from("/run/user/1000/procnetd.sock"));
    }

    #[test]
    fn socket_path_falls_back_to_system_when_no_xdg() {
        let p = resolve_socket_path(None);
        assert_eq!(p, PathBuf::from(SYSTEM_SOCKET_PATH));
    }

    #[test]
    fn socket_path_ignores_empty_xdg() {
        let p = resolve_socket_path(Some(OsStr::new("")));
        assert_eq!(p, PathBuf::from(SYSTEM_SOCKET_PATH));
    }
}
