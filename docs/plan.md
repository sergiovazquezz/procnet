# Simplified plan

## Distribution

- Binary for Ubuntu and Debian.
- Nix flake.
- Nix and Arch packages.
- AppImage?

## Known quirks

- Remove socket file on `SIGINT`/`SIGTERM`:

    ```rust
    fs::remove_file(socket)
    ```

- If a process exits and its PID is reused before its entry is removed from
  `StatsCollector`, for a tick the old name could be used to show the network
  usage of the new process. However it does not cause any data corruption other
  than displaying incorrect data for 1 tick.

- `stream.set_nonblocking(true)` + `set_write_timeout(Some(short))`; on
  `EAGAIN`/timeout, skip that client this tick (don't drop it); keep
  `retain_mut` for EOF. If deeper decoupling is ever needed, give each client a
  bounded writer thread.

## Tests

- `sort_rows` (`view.rs:17`) — test each `SortKey` both dirs + the `pid`
  tie-break.

## Features

- Add systemd service.

- Add udp with `udp_sendmsg`, `udp_recvmsg`, `udpv6_sendmsg` and
  `udpv6_recvmsg`.

- Use `log::error!()` for ebpf load.

- Add new pane for each process with the protocols used and cumulative stats.

- Add scrolling.

- Add pause for client.

### Arguments

- Global: `--version`.

- Daemon: `--socket`, `--interval`, `--stats-map-size`, `--events-size`,
  `--allow-any`, `--log-file`.

- Client: `--socket`. `DEFAULT_SOCKET_PATH` becomes the default.

## Possible features

- Add Unix sockets via `unix_stream_sendmsg` and `unix_stream_recvmsg`.
