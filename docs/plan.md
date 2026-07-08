# Simplified plan

## Distribution

- Binary for Ubuntu and Debian.
- Nix flake.
- Nix and Arch packages.
- AppImage?

## Known quirks

- If a process exits and its PID is reused before its entry is removed from
  `StatsCollector`, for a tick the old name could be used to show the network
  usage of the new process. However it does not cause any data corruption other
  than displaying incorrect data for 1 tick.

## Tests

- `sort_rows` (`view.rs:17`) — test each `SortKey` both dirs + the `pid`
  tie-break.

## Features

- Add systemd service.

- Add new pane for each process with the protocols used and cumulative stats.

- Add scrolling.

- Add pause for client.

### Arguments

- Stats: `--json`.

## Possible features

- Add Unix sockets via `unix_stream_sendmsg` and `unix_stream_recvmsg`.
