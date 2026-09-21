use std::{
    env, ffi,
    io::{self, BufRead, Read},
    os::unix::net::UnixStream,
    path::PathBuf,
};

use clap::{Subcommand, value_parser};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    errors::{ConnectError, MsgReadError, MsgSendError},
    stats::{ProcInfo, StatsRow},
};

/// Environment variable used to override the socket for development and tests.
const SOCKET_ENV_VAR: &str = "PROCNET_SOCKET";

/// Socket used by the installed system service.
const SYSTEM_SOCKET_PATH: &str = "/run/procnet/procnetd.sock";

/// Length of prefix used to frame each `bincode` message.
pub const PREFIX_LEN: usize = size_of::<u16>();

fn resolve_socket_path(configured_path: Option<&ffi::OsStr>) -> PathBuf {
    configured_path
        .filter(|path| !path.is_empty())
        .map_or_else(|| PathBuf::from(SYSTEM_SOCKET_PATH), PathBuf::from)
}

#[must_use]
pub fn socket_path() -> PathBuf {
    resolve_socket_path(env::var_os(SOCKET_ENV_VAR).as_deref())
}

pub fn connect_to_socket() -> Result<UnixStream, ConnectError> {
    UnixStream::connect(socket_path()).map_err(ConnectError)
}

#[derive(Clone, Copy, PartialEq, Eq, Subcommand, Serialize, Deserialize)]
pub enum DaemonCommand {
    Run,
    Interval {
        /// Daemon refresh interval in milliseconds (100ms - 5000ms).
        #[arg(value_parser = value_parser!(u64).range(100..=5000))]
        interval: u64,
    },
    IntervalIncrease,
    IntervalDecrease,
    Reset,
}

// NOTE: Serialize is only used for tests.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SnapshotData {
    pub interval: u64,
    pub tick: u64,
    pub rows: Vec<StatsRow>,
    pub dead_procs: Vec<ProcInfo>,
}

/// Borrowed view of a `SnapshotData`, used by the daemon to avoid cloning the
/// row buffer each tick.
#[derive(Serialize)]
pub struct SnapshotRef<'a> {
    pub interval: u64,
    pub tick: u64,
    pub rows: &'a [StatsRow],
    pub dead_procs: &'a [ProcInfo],
}

/// The resulting `buf` is `[len: u16 LE][payload: bincode]`.
pub fn write_msg<T: Serialize>(buf: &mut Vec<u8>, msg: &T) -> Result<(), MsgSendError> {
    buf.clear();
    buf.extend_from_slice(&[0u8; PREFIX_LEN]);

    bincode::serialize_into(&mut *buf, msg)?;

    let payload_len = u16::try_from(buf.len() - PREFIX_LEN).map_err(|_| MsgSendError::Oversized)?;
    buf[..PREFIX_LEN].copy_from_slice(&payload_len.to_le_bytes());

    Ok(())
}

pub fn read_msg<R, T>(reader: &mut R) -> Result<T, MsgReadError>
where
    R: BufRead,
    T: DeserializeOwned,
{
    let mut prefix_buf = [0u8; PREFIX_LEN];
    read_exact_or_eof(reader, &mut prefix_buf)?;

    let payload_len = u16::from_le_bytes(prefix_buf) as usize;
    let mut payload_buf = vec![0u8; payload_len];
    read_exact_or_eof(reader, &mut payload_buf)?;

    let response = bincode::deserialize_from(payload_buf.as_slice())?;

    Ok(response)
}

fn read_exact_or_eof<R: Read>(reader: &mut R, dest: &mut [u8]) -> Result<(), MsgReadError> {
    reader.read_exact(dest).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            MsgReadError::Eof
        } else {
            MsgReadError::Io(e)
        }
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::{
        assert_matches,
        ffi::OsStr,
        io::{BufReader, Cursor},
    };

    use crate::stats::{ProtocolStats, StatsAddr, StatsBytes};

    use super::*;

    #[test]
    fn socket_path_defaults_to_system_runtime_directory() {
        assert_eq!(
            resolve_socket_path(None),
            PathBuf::from("/run/procnet/procnetd.sock")
        );
    }

    #[test]
    fn socket_path_uses_configured_override() {
        assert_eq!(
            resolve_socket_path(Some(OsStr::new("/tmp/procnet-test.sock"))),
            PathBuf::from("/tmp/procnet-test.sock")
        );
    }

    #[test]
    fn socket_path_ignores_empty_override() {
        assert_eq!(
            resolve_socket_path(Some(OsStr::new(""))),
            PathBuf::from("/run/procnet/procnetd.sock")
        );
    }

    fn sample_rows() -> Vec<StatsRow> {
        let tcp_bytes = StatsBytes::new(512, 1024);
        let udp_bytes = StatsBytes::new(2048, 4096);

        vec![StatsRow::new(
            130,
            "firefox",
            ProtocolStats::new(tcp_bytes, StatsAddr::default()),
            ProtocolStats::new(udp_bytes, StatsAddr::default()),
            StatsBytes::default(),
            StatsBytes::default(),
        )]
    }

    fn sample_dead_procs() -> Vec<ProcInfo> {
        vec![ProcInfo::new(
            99,
            "ghost",
            StatsBytes { sent: 7, recv: 9 },
            StatsBytes { sent: 3, recv: 4 },
        )]
    }

    #[test]
    fn snapshot_ref_and_data_encode_identically() {
        let rows = sample_rows();
        let dead = sample_dead_procs();

        let owned = SnapshotData {
            interval: 1000,
            tick: 42,
            rows: rows.clone(),
            dead_procs: dead.clone(),
        };
        let borrowed = SnapshotRef {
            interval: 1000,
            tick: 42,
            rows: &rows,
            dead_procs: &dead,
        };

        assert_eq!(
            bincode::serialize(&owned).unwrap(),
            bincode::serialize(&borrowed).unwrap()
        );
    }

    #[test]
    fn write_msg_bytes_frames_with_le_length_prefix() {
        let rows = sample_rows();
        let dead = sample_dead_procs();
        let borrowed = SnapshotRef {
            interval: 100,
            tick: 42,
            rows: &rows,
            dead_procs: &dead,
        };
        let mut buf = Vec::<u8>::new();
        write_msg(&mut buf, &borrowed).unwrap();

        let prefix = u16::from_le_bytes(buf[..PREFIX_LEN].try_into().unwrap()) as usize;
        assert_eq!(prefix, buf.len() - PREFIX_LEN);

        let parsed: SnapshotData = bincode::deserialize_from(&buf[PREFIX_LEN..]).unwrap();
        assert_eq!(parsed.tick, 42);
        assert_eq!(parsed.rows, rows);
        assert_eq!(parsed.dead_procs, dead);
    }

    #[test]
    fn write_msg_bytes_reuses_buffer_capacity() {
        let rows = sample_rows();
        let dead = sample_dead_procs();
        let borrowed = SnapshotRef {
            interval: 5000,
            tick: 42,
            rows: &rows,
            dead_procs: &dead,
        };

        let mut buf = Vec::with_capacity(8192);

        write_msg(&mut buf, &borrowed).unwrap();

        let first_len = buf.len();

        write_msg(&mut buf, &borrowed).unwrap();

        assert_eq!(buf.len(), first_len);
    }

    #[test]
    fn read_msg_round_trips_frame() {
        let rows = sample_rows();
        let dead = sample_dead_procs();
        let borrowed = SnapshotRef {
            interval: 2500,
            tick: 7,
            rows: &rows,
            dead_procs: &dead,
        };
        let mut buf = Vec::new();
        write_msg(&mut buf, &borrowed).unwrap();

        let mut reader = BufReader::new(Cursor::new(buf));
        let parsed: SnapshotData = read_msg(&mut reader).unwrap();

        assert_eq!(parsed.interval, 2500);
        assert_eq!(parsed.tick, 7);
        assert_eq!(parsed.rows, rows);
        assert_eq!(parsed.dead_procs, dead);
    }

    #[test]
    fn read_msg_returns_eof_on_closed_stream() {
        let mut reader = BufReader::new(Cursor::new(Vec::<u8>::new()));
        let result = read_msg::<_, SnapshotData>(&mut reader);
        assert_matches!(result, Err(MsgReadError::Eof));
    }
}
