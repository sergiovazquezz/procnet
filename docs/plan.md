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

### Arguments

- Stats: `--json`.

## Possible features

- Add Unix sockets via `unix_stream_sendmsg` and `unix_stream_recvmsg`.
