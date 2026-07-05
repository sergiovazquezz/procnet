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

- Replace the `Vec<UnixStream>` with a thread per `UnixStream`. Use `mpsc` to
  send data to each thread which then updates the Stream.

## Tests

- `sort_rows` (`view.rs:17`) — test each `SortKey` both dirs + the `pid`
  tie-break.

## Features

- Add systemd service.

- Use `log::error!()` for ebpf load.

- Add new pane for each process with the protocols used and cumulative stats.

- Add scrolling.

- Add pause for client.

### Arguments

- Global: `--version`.

- Daemon: `--socket`, `--interval`, `--allow-any`, `--log-file`.

- Client: `--socket`. `DEFAULT_SOCKET_PATH` becomes the default.

## Possible features

- Add Unix sockets via `unix_stream_sendmsg` and `unix_stream_recvmsg`.
