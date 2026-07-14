# Plan

## Distribution

- Nix and Arch packages.
- AppImage?

## Known quirks

- If a process exits and its PID is reused before its entry is removed from
  `StatsCollector`, for a tick the old name could be used to show the network
  usage of the new process. However it does not cause any data corruption other
  than displaying incorrect data for 1 tick.

## Features

- Add a dead process section, with the possibility to remove any.
    - If a process with the same name exists merge the usage (optional).

- Show the IPs/ports a process has used.

- Add an option to change from cumulative stats to per tick stats.

- Instead of crashing the client when the daemon is not active or not working
  correctly, show a status message and reconnect when possible.

- Add a daemon pause 'P' from the TUI and CLI.

- Client: `--json`.

## Possible features

- A row with total network usage by active processes (maybe also dead).

- Add Unix sockets via `unix_stream_sendmsg` and `unix_stream_recvmsg`.

- Add reset to TUI.
