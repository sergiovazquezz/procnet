use std::{
    io::{self, BufRead, Read},
    os::unix::net::UnixStream,
};

use serde::{Deserialize, Serialize};

use crate::{
    errors::{ConnectError, MsgReadError, MsgSendError},
    stats::StatsRow,
};

pub const DEFAULT_SOCKET_PATH: &str = "/tmp/procnetd.sock";

/// Length of prefix used to frame each `bincode` message.
const PREFIX_LEN: usize = 4;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SnapshotData {
    pub tick: u64,
    pub rows: Vec<StatsRow>,
}

/// Borrowed view of a `SnapshotData`, used by the daemon to avoid cloning the
/// row buffer each tick.
#[derive(Serialize)]
pub struct SnapshotRef<'a> {
    pub tick: u64,
    pub rows: &'a [StatsRow],
}

pub fn connect_to_socket() -> Result<UnixStream, ConnectError> {
    let stream = UnixStream::connect(DEFAULT_SOCKET_PATH)?;
    Ok(stream)
}

/// The resulting `buf` is `[len: u32 LE][payload: bincode]`.
pub fn write_msg(buf: &mut Vec<u8>, msg: &SnapshotRef<'_>) -> Result<(), MsgSendError> {
    buf.clear();
    buf.extend_from_slice(&[0u8; PREFIX_LEN]);

    bincode::serialize_into(&mut *buf, msg)?;

    let payload_len = u32::try_from(buf.len() - PREFIX_LEN).map_err(|_| MsgSendError::Oversized)?;
    buf[..PREFIX_LEN].copy_from_slice(&payload_len.to_le_bytes());

    Ok(())
}

pub fn read_msg<R: BufRead>(reader: &mut R) -> Result<SnapshotData, MsgReadError> {
    let mut prefix_buf = [0u8; PREFIX_LEN];
    read_exact_or_eof(reader, &mut prefix_buf)?;

    let payload_len = u32::from_le_bytes(prefix_buf) as usize;
    let mut payload_buf = vec![0u8; payload_len];
    read_exact_or_eof(reader, &mut payload_buf)?;

    let response: SnapshotData = bincode::deserialize_from(payload_buf.as_slice())?;

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
    use std::io::{BufReader, Cursor};

    use crate::stats::StatsBytes;

    use super::*;

    fn sample_rows() -> Vec<StatsRow> {
        vec![StatsRow::new(
            130,
            "firefox".to_string(),
            StatsBytes {
                sent: 512,
                recv: 1024,
            },
            StatsBytes {
                sent: 2048,
                recv: 4096,
            },
        )]
    }

    #[test]
    fn snapshot_ref_and_data_encode_identically() {
        let rows = sample_rows();

        let owned = SnapshotData {
            tick: 42,
            rows: rows.clone(),
        };
        let borrowed = SnapshotRef {
            tick: 42,
            rows: &rows,
        };

        assert_eq!(
            bincode::serialize(&owned).unwrap(),
            bincode::serialize(&borrowed).unwrap()
        );
    }

    #[test]
    fn write_msg_bytes_frames_with_le_length_prefix() {
        let rows = sample_rows();
        let borrowed = SnapshotRef {
            tick: 42,
            rows: &rows,
        };
        let mut buf = Vec::<u8>::new();
        write_msg(&mut buf, &borrowed).unwrap();

        let prefix = u32::from_le_bytes(buf[..PREFIX_LEN].try_into().unwrap()) as usize;
        assert_eq!(prefix, buf.len() - PREFIX_LEN);

        let parsed: SnapshotData = bincode::deserialize_from(&buf[PREFIX_LEN..]).unwrap();
        assert_eq!(parsed.tick, 42);
        assert_eq!(parsed.rows, rows);
    }

    #[test]
    fn write_msg_bytes_reuses_buffer_capacity() {
        let rows = sample_rows();
        let borrowed = SnapshotRef {
            tick: 42,
            rows: &rows,
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
        let borrowed = SnapshotRef {
            tick: 7,
            rows: &rows,
        };
        let mut buf = Vec::new();
        write_msg(&mut buf, &borrowed).unwrap();

        let mut reader = BufReader::new(Cursor::new(buf));
        let parsed = read_msg(&mut reader).unwrap();

        assert_eq!(parsed.tick, 7);
        assert_eq!(parsed.rows, rows);
    }

    #[test]
    fn read_msg_returns_eof_on_closed_stream() {
        let mut reader = BufReader::new(Cursor::new(Vec::<u8>::new()));
        assert!(matches!(read_msg(&mut reader), Err(MsgReadError::Eof)));
    }
}
